use std::{io, path::Path};

use crate::project::config::ProjectConfig;

pub struct Project {
    #[expect(dead_code)]
    config: ProjectConfig,
}

impl Project {
    pub fn create_at(root_path: impl AsRef<Path>) -> io::Result<Self> {
        let root_path = root_path.as_ref();
        let config = ProjectConfig::try_create_at(root_path)?;
        Self::init_from_config(config)
    }

    pub fn open_at(path: impl AsRef<Path>) -> io::Result<Project> {
        let path = path.as_ref();
        let config = ProjectConfig::open_at(path)?;
        Self::init_from_config(config)
    }

    fn init_from_config(config: ProjectConfig) -> io::Result<Self> {
        Ok(Project { config })
    }
}
