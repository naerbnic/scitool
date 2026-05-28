use std::{
    cell::OnceCell,
    collections::BTreeMap,
    io::Read,
    path::{Path, PathBuf},
};

use anyhow::Context as _;
use scidev::{
    resources::{ResourceSet, ResourceType, types::msg::parse_message_resource},
    utils::serde::Sha256Hash,
};
use sciproj::{
    book::{Book, builder::BookBuilder, config::BookConfig},
    path::relpath::{RelPath, RelPathBuf},
};
use serde::{Deserialize, Serialize};

use crate::{
    commands::config::ProjectConfig,
    data::{ConfigFormat, load_config},
};

const PROJECT_FILE_NAME: &str = "scidub.toml";
const MANIFEST_FILE_NAME: &str = "scidub.manifest.json";

pub(crate) fn find_project_root(start_path: impl AsRef<Path>) -> anyhow::Result<PathBuf> {
    let canon_path = start_path.as_ref().canonicalize()?;
    let mut current_path: &Path = &canon_path;
    anyhow::ensure!(
        current_path.exists(),
        "Can't find project from nonexistent path: {}",
        current_path.display()
    );
    loop {
        let possible_project_file = current_path.join(PROJECT_FILE_NAME);
        if possible_project_file.is_file() {
            return Ok(current_path.to_path_buf());
        }
        current_path = current_path.parent().ok_or_else(|| {
            anyhow::anyhow!(
                "Reached root directory without finding project file {PROJECT_FILE_NAME}"
            )
        })?;
    }
}

fn get_or_init<T, F, E>(cell: &OnceCell<T>, init: F) -> Result<&T, E>
where
    F: FnOnce() -> Result<T, E>,
{
    let value = if let Some(value) = cell.get() {
        value
    } else {
        let value = init()?;
        assert!(cell.set(value).is_ok(), "should be empty");
        cell.get().unwrap()
    };

    Ok(value)
}

pub(crate) struct Project {
    root: PathBuf,
    config_path: OnceCell<PathBuf>,
    config: OnceCell<ProjectConfig>,
    build_dir: OnceCell<PathBuf>,
    manifest_path: OnceCell<PathBuf>,
    manifest: OnceCell<Option<Manifest>>,
    game_path: OnceCell<Option<PathBuf>>,
    resources: OnceCell<Option<ResourceSet>>,
    book_config_path: OnceCell<Option<PathBuf>>,
    book_config: OnceCell<Option<BookConfig>>,
    book_opt: OnceCell<Option<Book>>,
}

impl Project {
    pub(crate) fn new_from_path(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let root = find_project_root(path)?;
        Ok(Self::new(root))
    }

    pub(crate) fn new(root: PathBuf) -> Self {
        Project {
            root,
            config_path: OnceCell::new(),
            config: OnceCell::new(),
            build_dir: OnceCell::new(),
            manifest_path: OnceCell::new(),
            manifest: OnceCell::new(),
            game_path: OnceCell::new(),
            resources: OnceCell::new(),
            book_config_path: OnceCell::new(),
            book_config: OnceCell::new(),
            book_opt: OnceCell::new(),
        }
    }

    pub(crate) fn root(&self) -> &Path {
        self.root.as_path()
    }

    pub(crate) fn config_path(&self) -> anyhow::Result<&Path> {
        let path = get_or_init(&self.config_path, || {
            let path = self.root.join(PROJECT_FILE_NAME);
            anyhow::ensure!(path.is_file(), "Project file not found");
            Ok(path)
        })?;
        Ok(path.as_path())
    }

    pub(crate) fn manifest_path(&self) -> anyhow::Result<&Path> {
        let path = get_or_init(&self.manifest_path, || {
            Ok::<_, anyhow::Error>(self.root.join(MANIFEST_FILE_NAME))
        })?;
        Ok(path)
    }

    pub(crate) fn manifest_opt(&self) -> anyhow::Result<Option<&Manifest>> {
        let manifest_opt = get_or_init(&self.manifest, || {
            let path = self.manifest_path()?;
            if !path.is_file() {
                return Ok(None);
            }
            let manifest: Manifest = serde_json::from_reader(std::fs::File::open(path)?)
                .context(format!("Unable to parse manifest file {}", path.display()))?;
            Ok::<_, anyhow::Error>(Some(manifest))
        })?;
        Ok(manifest_opt.as_ref())
    }

    pub(crate) fn manifest(&self) -> anyhow::Result<&Manifest> {
        self.manifest_opt()?
            .ok_or_else(|| anyhow::anyhow!("Manifest not found"))
    }

    pub(crate) fn config(&self) -> anyhow::Result<&ProjectConfig> {
        get_or_init(&self.config, || {
            let config_path = self.config_path()?;
            let config: ProjectConfig = toml::from_slice(&std::fs::read(config_path).context(
                anyhow::format_err!("Unable to open project file {}", config_path.display()),
            )?)
            .context(format!(
                "Unable to parse project file {}",
                config_path.display()
            ))?;
            Ok(config)
        })
    }

    pub(crate) fn build_dir(&self) -> anyhow::Result<&Path> {
        let build_dir = get_or_init(&self.build_dir, || {
            let config = self.config()?;
            let config_build_dir = config.build_dir();
            let rel_path = config_build_dir.unwrap_or(RelPath::new("build"));
            Ok::<_, anyhow::Error>(rel_path.to_std_path(self.root()))
        })?;

        Ok(build_dir)
    }

    pub(crate) fn game_path_opt(&self) -> anyhow::Result<Option<&Path>> {
        Ok(get_or_init(&self.game_path, || {
            let config = self.config()?;
            let Some(game_path) = config.game_files() else {
                return Ok(None);
            };
            Ok::<_, anyhow::Error>(Some(game_path.to_std_path(self.root())))
        })?
        .as_deref())
    }

    pub(crate) fn game_path(&self) -> anyhow::Result<&Path> {
        self.game_path_opt()?
            .ok_or_else(|| anyhow::anyhow!("No game path configured for the project"))
    }

    pub(crate) fn resources(&self) -> anyhow::Result<Option<&ResourceSet>> {
        Ok(get_or_init(&self.resources, || {
            let Some(game_path) = self.game_path_opt()? else {
                return Ok(None);
            };

            let resource_set = ResourceSet::from_root_dir(game_path)?;
            Ok::<_, anyhow::Error>(Some(resource_set))
        })?
        .as_ref())
    }

    pub(crate) fn book_config_path(&self) -> anyhow::Result<Option<&Path>> {
        Ok(get_or_init(&self.book_config_path, || {
            let config = self.config()?;
            let Some(book_config_path) = config.voice_script_config() else {
                return Ok(None);
            };
            Ok::<_, anyhow::Error>(Some(book_config_path.to_std_path(self.root())))
        })?
        .as_deref())
    }

    pub(crate) fn book_config(&self) -> anyhow::Result<Option<&BookConfig>> {
        Ok(get_or_init(&self.book_config, || {
            let Some(book_config_path) = self.book_config_path()? else {
                return Ok(None);
            };
            let script_config: BookConfig = load_config(book_config_path, &ConfigFormat::Toml)?;
            Ok::<_, anyhow::Error>(Some(script_config))
        })?
        .as_ref())
    }

    pub(crate) fn book_opt(&self) -> anyhow::Result<Option<&Book>> {
        Ok(get_or_init(&self.book_opt, || {
            let Some(resource_set) = self.resources()? else {
                return Ok(None);
            };

            let Some(book_config) = self.book_config()? else {
                return Ok(None);
            };

            let mut builder = BookBuilder::new(book_config.clone())?;

            for res in resource_set.resources_of_type(ResourceType::Message) {
                let msg_resources = parse_message_resource(&res.data().open_mem(..)?)?;
                for (msg_id, record) in msg_resources.messages() {
                    builder.add_message(res.id().resource_num(), msg_id, record)?;
                }
            }

            Ok::<_, anyhow::Error>(Some(builder.build()?))
        })?
        .as_ref())
    }

    pub(crate) fn book(&self) -> anyhow::Result<&Book> {
        self.book_opt()?
            .ok_or_else(|| anyhow::anyhow!("No book is configured for this project"))
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct Manifest(BTreeMap<RelPathBuf, Sha256Hash>);

impl Manifest {
    pub(crate) fn new() -> Self {
        Self(BTreeMap::new())
    }

    pub(crate) fn add(&mut self, path: impl AsRef<RelPath>, hash: Sha256Hash) {
        self.0.insert(path.as_ref().to_buf(), hash);
    }

    pub(crate) fn add_from_reader<R>(
        &mut self,
        path: impl AsRef<RelPath>,
        reader: R,
    ) -> anyhow::Result<()>
    where
        R: Read,
    {
        let (hash, _) = Sha256Hash::from_stream_hash(reader)?;
        self.add(path, hash);
        Ok(())
    }

    pub(crate) fn match_file<R>(&self, path: impl AsRef<RelPath>, data: R) -> anyhow::Result<bool>
    where
        R: Read,
    {
        let Some(hash) = self.0.get(path.as_ref()) else {
            return Ok(false);
        };

        let (file_hash, _) = Sha256Hash::from_stream_hash(data)?;
        Ok(file_hash == *hash)
    }

    pub(crate) fn entries(&self) -> &BTreeMap<RelPathBuf, Sha256Hash> {
        &self.0
    }
}
