use super::{
    lifecycle::CompletedOutput,
    plan::EncodePlan,
    progress::{BarUpdate, ProgressState, StreamSizes, apply_ffmpeg_event},
    sink::ProgressSink,
    spawner::EncodeSpawner,
};
use crate::{command::SmallDuration, ffmpeg, ffprobe, log::ProgressLogger};
use anyhow::ensure;
use log::info;
use std::time::{Duration, Instant};
use tokio_stream::StreamExt;

const VERIFY_DURATION_TOLERANCE: Duration = Duration::from_secs(2);
const VERIFY_BAR_DIVISOR: u64 = 2;

pub struct EncodeRun {
    pub input: std::path::PathBuf,
    pub output: CompletedOutput,
    pub stream_sizes: Option<StreamSizes>,
}

pub async fn run_encode(
    plan: EncodePlan,
    sink: &impl ProgressSink,
    spawner: &impl EncodeSpawner,
) -> anyhow::Result<EncodeRun> {
    run_encode_with_progress(plan, sink, spawner, |_fps, _time, _output| {}).await
}

pub async fn run_encode_with_progress<F>(
    plan: EncodePlan,
    sink: &impl ProgressSink,
    spawner: &impl EncodeSpawner,
    mut on_progress: F,
) -> anyhow::Result<EncodeRun>
where
    F: FnMut(f32, std::time::Duration, &std::path::Path),
{
    let (partial, session) = plan.begin()?;
    let input = session.input.clone();
    let output = partial.path().to_path_buf();

    info!(
        "encoding {}",
        output.file_name().and_then(|n| n.to_str()).unwrap_or("")
    );

    let enc_args = session.ffmpeg_args()?;
    let mut enc = spawner.spawn(&session, enc_args, &partial)?;

    let mut logger = ProgressLogger::new(module_path!(), Instant::now());
    let mut progress = ProgressState::default();
    while let Some(event) = enc.next().await {
        let event = event?;
        if let Some(BarUpdate::Fps { frame, fps, time }) = apply_ffmpeg_event(&mut progress, event)
        {
            let time = media_time(
                frame,
                time,
                session.probe.fps.as_ref().copied().unwrap_or_default(),
            );
            sink.set_message(format!("{fps} fps, "));
            if let Ok(d) = &session.probe.duration {
                sink.set_position(time.as_micros_u64());
                logger.update(*d, time, fps);
            }
            on_progress(fps, time, &output);
        }
    }
    enc.wait().await?;

    // Verify while the output is still staged, so a failed check cannot replace
    // an existing destination.
    if session.verify_decode() {
        sink.set_message("verifying, ");
        let encode_len = session
            .probe
            .duration
            .as_ref()
            .ok()
            .map(|d| d.as_micros_u64());
        ffmpeg::decode(&output, |event| {
            if let crate::process::FfmpegOut::Progress { fps, time, .. } = event {
                sink.set_message(if fps > 0.0 {
                    format!("verifying {fps} fps, ")
                } else {
                    "verifying, ".into()
                });
                if let Some(len) = encode_len {
                    sink.set_position(len + time.as_micros_u64() / VERIFY_BAR_DIVISOR);
                }
            }
        })
        .await?;
    }
    if session.verify_duration()
        && let Ok(expected) = &session.probe.duration
        && !expected.is_zero()
    {
        let actual = ffprobe::probe(&output).duration?;
        ensure!(
            expected.abs_diff(actual) <= VERIFY_DURATION_TOLERANCE,
            "verify: output duration {} does not match input duration {}",
            humantime::format_duration(floor_ms(actual)),
            humantime::format_duration(floor_ms(*expected)),
        );
    }
    sink.finish();

    spawner.finalize_output(&output).await?;

    Ok(EncodeRun {
        input,
        output: partial.commit()?,
        stream_sizes: progress.stream_sizes,
    })
}

fn floor_ms(duration: Duration) -> Duration {
    Duration::from_millis(duration.as_millis().try_into().unwrap_or(u64::MAX))
}

fn media_time(frame: u64, reported: Duration, source_fps: f64) -> Duration {
    if reported.is_zero() && source_fps > 0.0 {
        Duration::from_secs_f64(frame as f64 / source_fps)
    } else {
        reported
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_media_time_from_frame_when_ffmpeg_reports_na() {
        assert_eq!(
            media_time(240, Duration::ZERO, 24.0),
            Duration::from_secs(10)
        );
    }
}
