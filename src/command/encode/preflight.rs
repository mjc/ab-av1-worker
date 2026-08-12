use super::{default_output_name, error::EncodePlanError, lifecycle::PlannedOutput};
use crate::{command::args, ffprobe::Ffprobe};
use same_file::is_same_file;
use std::{path::Path, sync::Arc};

/// Resolved output path and whether the CLI omitted `--output`.
#[derive(Debug)]
pub(in crate::command) struct ResolvedOutput {
    pub(in crate::command) planned: PlannedOutput,
    pub(in crate::command) defaulting_output: bool,
}

/// Audio options derived from CLI flags and probe metadata.
#[derive(Debug)]
pub(in crate::command) struct AudioConfig {
    pub(super) has_audio: bool,
    pub(super) stereo_downmix: bool,
    pub(super) audio_codec: Option<Arc<str>>,
}

pub(in crate::command) fn resolve_output(
    input: &Path,
    encoder: &args::Encoder,
    encode_to: &args::EncodeToOutput,
    probe: &Ffprobe,
) -> Result<ResolvedOutput, EncodePlanError> {
    let defaulting_output = encode_to.output.is_none();
    let output_path = encode_to
        .output
        .clone()
        .unwrap_or_else(|| default_output_name(input, encoder, probe.is_image));

    if !encode_to.overwrite_input && is_same_file(&output_path, input).unwrap_or(false) {
        return Err(EncodePlanError::SameInputOutput);
    }

    Ok(ResolvedOutput {
        planned: PlannedOutput::new(output_path),
        defaulting_output,
    })
}

pub(in crate::command) fn audio_config(
    encode_to: &args::EncodeToOutput,
    probe: &Ffprobe,
) -> Result<AudioConfig, EncodePlanError> {
    if encode_to.downmix_to_stereo && encode_to.audio_codec.as_deref() == Some("copy") {
        return Err(EncodePlanError::StereoDownmixWithCopy);
    }
    let stereo_downmix =
        encode_to.downmix_to_stereo && probe.max_audio_channels.is_some_and(|c| c > 3);
    let audio_codec = encode_to.audio_codec.clone().map(Into::into);
    Ok(AudioConfig {
        has_audio: probe.has_audio,
        stereo_downmix,
        audio_codec,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{
        args::{EncodeToOutput, Encoder},
        encode::test_support::{temp_input, test_probe},
    };
    use std::fs;

    #[test]
    fn resolve_output_rejects_same_input_and_output() {
        let input = temp_input("preflight", "same-io");
        let encode_to = EncodeToOutput {
            output: Some(input.clone()),
            audio_codec: None,
            downmix_to_stereo: false,
            video_only: false,
            overwrite_input: false,
        };
        let encoder: Encoder = "libsvtav1".parse().unwrap();
        let err = match resolve_output(&input, &encoder, &encode_to, &test_probe(Some(6))) {
            Err(err) => err,
            Ok(_) => panic!("expected same-file error"),
        };
        assert_eq!(err, EncodePlanError::SameInputOutput);
        let _ = fs::remove_file(input);
    }

    #[test]
    fn audio_config_rejects_stereo_downmix_with_copy() {
        let encode_to = EncodeToOutput {
            output: None,
            audio_codec: Some("copy".into()),
            downmix_to_stereo: true,
            video_only: false,
            overwrite_input: false,
        };
        let err = match audio_config(&encode_to, &test_probe(Some(6))) {
            Err(err) => err,
            Ok(_) => panic!("expected error"),
        };
        assert_eq!(err, EncodePlanError::StereoDownmixWithCopy);
    }
}
