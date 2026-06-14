use std::{collections::BTreeMap, path::PathBuf};

pub mod espeak;
pub mod ffmpeg;

mod util;

#[derive(Debug)]
pub struct Tool {
    binary_path: PathBuf,
    prefix_args: Vec<String>,
    env: BTreeMap<String, String>,
}

impl Tool {
    #[must_use]
    pub fn from_path(path: PathBuf) -> Self {
        Tool {
            binary_path: path,
            prefix_args: vec![],
            env: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn with_prefix_args(mut self, args: impl IntoIterator<Item = String>) -> Self {
        self.prefix_args.extend(args);
        self
    }

    #[must_use]
    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    #[must_use]
    pub fn cmd_async(&self) -> tokio::process::Command {
        let mut cmd = tokio::process::Command::new(&self.binary_path);
        cmd.envs(&self.env).args(&self.prefix_args);
        cmd
    }
}
