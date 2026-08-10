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
    guard: CleanupGuard,
    final_path: PathBuf,
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
        let final_path = self.path;
        let mut staged_name = OsString::from(".tmp.ab-av1-encoding.");
        staged_name.push(final_path.file_name().context("no output file name")?);
        let staged_path = final_path.with_file_name(staged_name);
        Ok(PartialOutput {
            guard: CleanupGuard::arm(staged_path),
            final_path,
        })
    }
}

impl PartialOutput {
    pub fn path(&self) -> &Path {
        self.guard.path()
    }

    pub fn commit(self) -> anyhow::Result<CompletedOutput> {
        let Self { guard, final_path } = self;
        std::fs::rename(guard.path(), &final_path).with_context(|| {
            format!(
                "move encoded output {} to {}",
                guard.path().display(),
                final_path.display()
            )
        })?;
        guard.disarm();
        Ok(CompletedOutput { path: final_path })
    }
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
        let staged;
        {
            let partial = PlannedOutput::new(path.clone())
                .begin()
                .expect("begin output");
            staged = partial.path().to_path_buf();
            fs::write(&staged, b"temp").expect("write staged file");
        }
        temporary::clean_all().await;
        assert!(
            !staged.exists(),
            "uncommitted partial output must be deleted"
        );
    }

    #[serial]
    #[tokio::test]
    async fn completed_output_replaces_destination_and_survives_cleanup() {
        let path = temp_path("completed");
        fs::write(&path, b"old").expect("write final file");
        let partial = PlannedOutput::new(path.clone())
            .begin()
            .expect("begin output");
        assert_eq!(partial.path().parent(), path.parent());
        assert!(
            partial
                .path()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(".tmp.ab-av1-encoding.")
        );
        fs::write(partial.path(), b"new").expect("write staged file");
        let completed = partial.commit().expect("commit output");
        temporary::clean_all().await;
        assert_eq!(completed.path(), path);
        assert_eq!(fs::read(&path).expect("read final file"), b"new");
        let _ = fs::remove_file(path);
    }
}
