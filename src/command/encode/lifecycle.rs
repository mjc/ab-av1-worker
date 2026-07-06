use crate::temporary::CleanupGuard;
use anyhow::Context;
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

/// User-selected or defaulted output path before ffmpeg may write it.
#[must_use]
#[derive(Debug)]
pub struct PlannedOutput {
    path: PathBuf,
}

/// Output registered for cleanup while ffmpeg may be writing it.
#[must_use]
pub struct PartialOutput {
    output: PathBuf,
    guard: CleanupGuard,
}

/// Successful encode output, no longer subject to failure cleanup.
#[must_use]
pub struct CompletedOutput {
    path: PathBuf,
}

impl PlannedOutput {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn begin(self) -> anyhow::Result<PartialOutput> {
        let temp = temporary_output_path(&self.path)?;
        Ok(PartialOutput {
            output: self.path,
            guard: CleanupGuard::arm(temp),
        })
    }
}

impl PartialOutput {
    pub fn path(&self) -> &Path {
        self.guard.path()
    }

    pub fn output_path(&self) -> &Path {
        &self.output
    }

    pub fn commit(self) -> anyhow::Result<CompletedOutput> {
        std::fs::rename(self.guard.path(), &self.output)
            .with_context(|| format!("move completed encode to {}", self.output.display()))?;
        let _ = self.guard.disarm();
        Ok(CompletedOutput { path: self.output })
    }
}

fn temporary_output_path(output: &Path) -> anyhow::Result<PathBuf> {
    let mut temp_name = OsString::from(".tmp.ab-av1-encoding.");
    temp_name.push(output.file_name().context("no output file name")?);
    let mut temp = output.to_path_buf();
    temp.set_file_name(temp_name);
    Ok(temp)
}

impl crate::ffmpeg::EncodeDestination for PartialOutput {
    fn encode_destination(&self) -> &Path {
        self.path()
    }
}

impl CompletedOutput {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::temporary;
    use serial_test::serial;
    use std::{env, fs};

    fn temp_path(label: &str) -> PathBuf {
        env::temp_dir().join(format!(
            "ab-av1-encode-lifecycle-{}-{}",
            label,
            std::process::id()
        ))
    }

    #[serial]
    #[test]
    fn planned_output_is_not_registered_for_cleanup() {
        let path = temp_path("planned");
        fs::write(&path, b"stay").expect("write file");
        let _planned = PlannedOutput::new(path.clone());
        assert!(
            !temporary::unadd(&path),
            "planned output must not register cleanup"
        );
        let _ = fs::remove_file(path);
    }

    #[serial]
    #[tokio::test]
    async fn partial_output_cleans_up_when_not_committed() {
        let path = temp_path("partial-drop");
        {
            let partial = PlannedOutput::new(path.clone())
                .begin()
                .expect("begin output");
            fs::write(partial.path(), b"temp").expect("write partial");
        }
        temporary::clean_all().await;
        assert!(!path.exists(), "uncommitted partial output must be deleted");
    }

    #[serial]
    #[tokio::test]
    async fn completed_output_survives_cleanup() {
        let path = temp_path("completed");
        let partial = PlannedOutput::new(path.clone())
            .begin()
            .expect("begin output");
        fs::write(partial.path(), b"stay").expect("write partial");
        let completed = partial.commit().expect("commit output");
        temporary::clean_all().await;
        assert!(
            completed.path().exists(),
            "completed output must survive cleanup"
        );
        let _ = fs::remove_file(path);
    }
}
