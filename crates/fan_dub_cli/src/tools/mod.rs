use std::{ffi::OsStr, path::PathBuf};

pub mod ffmpeg;

pub struct Tool {
    binary_path: PathBuf,
}

impl Tool {
    #[must_use]
    pub fn from_path(path: PathBuf) -> Self {
        Tool { binary_path: path }
    }

    pub fn run<'args>(
        &self,
        args: impl IntoIterator<Item = &'args (impl AsRef<OsStr> + 'args)>,
        envs: impl IntoIterator<
            Item = (
                &'args (impl AsRef<OsStr> + 'args),
                &'args (impl AsRef<OsStr> + 'args),
            ),
        >,
    ) -> anyhow::Result<()> {
        let mut command = std::process::Command::new(&self.binary_path);
        command.args(args);
        command.envs(envs);
        Ok(())
    }
}
