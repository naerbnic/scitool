use std::{
    collections::BTreeMap,
    io::{self, Write as _},
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use walkdir::WalkDir;

pub(crate) mod schema;

const ROOT_CONFIG_FILE_NAME: &str = "sciproj.toml";
const DIR_CONFIG_FILE_NAME: &str = "dir.sciproj.toml";

fn find_root_config_path(path: &Path) -> io::Result<PathBuf> {
    let initial_path = path.canonicalize()?;
    if !initial_path.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Initial path must be a directory",
        ));
    }

    for path in initial_path.ancestors() {
        let potential_config_path = path.join(ROOT_CONFIG_FILE_NAME);
        let metadata = potential_config_path.metadata()?;
        if metadata.is_file() {
            return Ok(path.to_path_buf());
        }
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "Could not find root config file",
    ))
}

fn read_config<T>(path: &Path) -> io::Result<T>
where
    T: DeserializeOwned,
{
    let data = std::fs::read_to_string(path)?;
    let contents =
        toml::from_str(&data).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok(contents)
}

fn write_new(path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> io::Result<()> {
    std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)?
        .write_all(contents.as_ref())?;
    Ok(())
}

/// The root config file for sciproj.
#[expect(dead_code, reason = "in progress")]
pub(crate) struct RootConfig {
    contents: schema::ProjectConfig,
}

impl RootConfig {
    #[expect(dead_code, reason = "in progress")]
    pub(crate) fn open_at(path: &impl AsRef<Path>) -> io::Result<Self> {
        let data = std::fs::read_to_string(path)?;
        let contents =
            toml::from_str(&data).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        Ok(RootConfig { contents })
    }
}

/// A representation of the raw data behind the workspace environment.
pub(crate) struct ProjectConfig {
    #[expect(dead_code, reason = "in progress")]
    root_path: PathBuf,
    #[expect(dead_code, reason = "in progress")]
    root: schema::ProjectConfig,
    #[expect(dead_code, reason = "in progress")]
    dir_configs: BTreeMap<PathBuf, schema::DirConfig>,
}

impl ProjectConfig {
    pub(crate) fn try_create_at(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref();
        match find_root_config_path(path) {
            Ok(path) => Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!(
                    "sciproj.toml already exists at {}",
                    path.join(ROOT_CONFIG_FILE_NAME).display()
                ),
            )),
            Err(e) if matches!(e.kind(), io::ErrorKind::NotFound) => Self::create_at(path),
            Err(e) => Err(e),
        }
    }

    fn create_at(path: &Path) -> io::Result<Self> {
        std::fs::create_dir_all(path)?;
        write_new(
            path.join(ROOT_CONFIG_FILE_NAME),
            include_str!("defaults/sciproj.toml.tmpl"),
        )?;
        Self::open_at_root(path)
    }

    fn open_at_root(root_dir: &Path) -> io::Result<Self> {
        let root_config_file_path = root_dir.join(ROOT_CONFIG_FILE_NAME);
        let mut dir_configs = Vec::new();
        for entry in WalkDir::new(root_dir) {
            let entry = entry?;
            if entry.path().ends_with(DIR_CONFIG_FILE_NAME) && entry.file_type().is_file() {
                dir_configs.push(entry.path().to_path_buf());
            }
        }

        let dir_configs = dir_configs
            .into_iter()
            .map(|mut path| {
                let config = read_config(&path)?;
                path.pop();
                Ok((path, config))
            })
            .collect::<io::Result<_>>()?;

        Ok(ProjectConfig {
            root_path: root_dir.to_path_buf(),
            root: read_config(&root_config_file_path)?,
            dir_configs,
        })
    }
    pub(crate) fn open_at(path: impl AsRef<Path>) -> io::Result<Self> {
        let root_dir = find_root_config_path(path.as_ref())?;
        Self::open_at_root(&root_dir)
    }
}
