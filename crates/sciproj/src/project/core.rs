use std::{
    io::{self, Write as _},
    path::{Path, PathBuf},
};

use crate::project::{config::ConfigFile, state::StateFile};

const CONFIG_FILE_NAME: &str = "sciproj.toml";
const STATE_FILE_NAME: &str = "sciproj.state.json";

fn write_new(path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> io::Result<()> {
    std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)?
        .write_all(contents.as_ref())?;
    Ok(())
}

pub struct Project {
    #[expect(dead_code)]
    root_path: PathBuf,
    #[expect(dead_code)]
    config: ConfigFile,
    #[expect(dead_code)]
    state: StateFile,
}

impl Project {
    pub fn create_at(root_path: impl AsRef<Path>) -> io::Result<Self> {
        let root_path = root_path.as_ref();
        std::fs::create_dir_all(root_path)?;
        write_new(
            root_path.join(CONFIG_FILE_NAME),
            include_str!("defaults/sciproj.toml.tmpl"),
        )?;
        write_new(
            root_path.join(STATE_FILE_NAME),
            include_str!("defaults/sciproj.state.json.tmpl"),
        )?;
        Self::open_at_root(root_path)
    }

    pub fn open_at_root(root_path: impl AsRef<Path>) -> io::Result<Project> {
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
