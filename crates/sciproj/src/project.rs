use std::{
    io,
    path::{Path, PathBuf},
};

use crate::{config::ConfigFile, state::StateFile};

const CONFIG_FILE_NAME: &str = "sciproj.toml";
const STATE_FILE_NAME: &str = "sciproj.state.json";

pub struct Project {
    #[expect(dead_code)]
    root_path: PathBuf,
    #[expect(dead_code)]
    config: ConfigFile,
    #[expect(dead_code)]
    state: StateFile,
}

impl Project {
    pub fn open_at_root(root_path: &impl AsRef<Path>) -> io::Result<Project> {
        let root_path = root_path.as_ref();
        let config = ConfigFile::open_at(&root_path.join(CONFIG_FILE_NAME))?;
        let state = StateFile::open_at(&root_path.join(STATE_FILE_NAME))?;
        Ok(Project {
            root_path: root_path.to_path_buf(),
            config,
            state,
        })
    }
}
