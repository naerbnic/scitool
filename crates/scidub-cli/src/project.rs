mod artifacts;

use std::{
    collections::BTreeMap,
    io::Read,
    path::{Path, PathBuf},
};

use scidev::utils::serde::Sha256Hash;
use scidev_inc::Universe;
use sciproj::{
    book::Book,
    path::{
        abspath::{AbsPath, AbsPathBuf},
        relpath::{RelPath, RelPathBuf},
        walkdir::RelWalkDir,
    },
};
use serde::{Deserialize, Serialize};

use crate::commands::config::ProjectConfig;

const PROJECT_FILE_NAME: &str = "scidub.toml";
const MANIFEST_FILE_NAME: &str = "scidub.manifest.json";
const SCRIPTS_DEFAULT_PATH: &str = "scripts";

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

pub(crate) struct Project {
    universe: Universe,
}

impl Project {
    pub(crate) fn new_from_path(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let root = find_project_root(path)?;
        let mut universe = Universe::new();
        universe.add_input(artifacts::Root, AbsPathBuf::from_std(root));
        Ok(Self { universe })
    }

    pub(crate) fn root(&self) -> &Path {
        self.universe.get_input(artifacts::Root).as_std()
    }

    pub(crate) fn manifest_opt(&self) -> anyhow::Result<Option<&Manifest>> {
        Ok(self
            .universe
            .sync_op(artifacts::manifest_opt)
            .can_fail_disp()
            .get_sync()?
            .as_ref())
    }

    pub(crate) fn manifest(&self) -> anyhow::Result<&Manifest> {
        let Some(manifest) = self.manifest_opt()? else {
            return Err(anyhow::anyhow!("Manifest not found"));
        };
        Ok(manifest)
    }

    fn game_path_opt(&self) -> anyhow::Result<Option<&AbsPath>> {
        Ok(self
            .universe
            .sync_op(artifacts::game_path_opt)
            .can_fail_disp()
            .get_sync()?
            .as_deref())
    }

    pub(crate) fn game_path(&self) -> anyhow::Result<&Path> {
        Ok(self
            .game_path_opt()?
            .ok_or_else(|| anyhow::anyhow!("No game path configured for the project"))?
            .as_std())
    }

    fn book_opt(&self) -> anyhow::Result<Option<&Book>> {
        Ok(self
            .universe
            .sync_op(artifacts::book_opt)
            .can_fail_disp()
            .get_sync()?
            .as_ref())
    }

    pub(crate) fn book(&self) -> anyhow::Result<&Book> {
        self.book_opt()?
            .ok_or_else(|| anyhow::anyhow!("No book is configured for this project"))
    }

    pub(crate) fn build_dir(&self) -> anyhow::Result<&AbsPath> {
        Ok(self
            .universe
            .sync_op(artifacts::build_dir)
            .can_fail_disp()
            .get_sync()?)
    }

    pub(crate) fn config(&self) -> anyhow::Result<&ProjectConfig> {
        Ok(self.universe.op(artifacts::project_config))
    }
}

// #[task_struct]
// mod project_impl {
//     #[derive(Debug)]
//     pub(crate) struct Project {
//         pub(super) root: PathBuf,
//     }

//     impl Project {
//         #[result]
//         pub(crate) fn config_path(&self) -> anyhow::Result<PathBuf> {
//             let path = self.root.join(PROJECT_FILE_NAME);
//             anyhow::ensure!(path.is_file(), "Project file not found");
//             Ok(path)
//         }

//         pub(crate) fn manifest_path(&self) -> PathBuf {
//             self.root.join(MANIFEST_FILE_NAME)
//         }

//         #[result]
//         pub(crate) fn manifest_opt(&self) -> anyhow::Result<Option<Manifest>> {
//             let path = self.manifest_path();
//             if !path.is_file() {
//                 return Ok(None);
//             }
//             let manifest: Manifest = serde_json::from_reader(std::fs::File::open(path)?)
//                 .context(format!("Unable to parse manifest file {}", path.display()))?;
//             Ok::<_, anyhow::Error>(Some(manifest))
//         }

//         #[result]
//         pub(crate) fn config(&self) -> anyhow::Result<ProjectConfig> {
//             let config_path = self.config_path()?;
//             let config: ProjectConfig = toml::from_slice(&std::fs::read(config_path).context(
//                 anyhow::format_err!("Unable to open project file {}", config_path.display()),
//             )?)
//             .context(format!(
//                 "Unable to parse project file {}",
//                 config_path.display()
//             ))?;
//             Ok(config)
//         }

//         #[result]
//         pub(crate) fn build_dir(&self) -> anyhow::Result<PathBuf> {
//             let config = self.config()?;
//             let config_build_dir = config.build_dir();
//             let rel_path = config_build_dir.unwrap_or(RelPath::new("build"));
//             Ok::<_, anyhow::Error>(rel_path.to_std_path(self.root()))
//         }

//         #[result]
//         pub(crate) fn game_path_opt(&self) -> anyhow::Result<Option<PathBuf>> {
//             let config = self.config()?;
//             let Some(game_path) = config.game_files() else {
//                 return Ok(None);
//             };
//             Ok(Some(game_path.to_std_path(self.root())))
//         }

//         #[result]
//         pub(crate) fn resources_opt(&self) -> anyhow::Result<Option<ResourceSet>> {
//             let Some(game_path) = self.game_path_opt()? else {
//                 return Ok(None);
//             };

//             let resource_set = ResourceSet::from_root_dir(game_path)?;
//             Ok(Some(resource_set))
//         }

//         #[result]
//         pub(crate) fn book_config_path(&self) -> anyhow::Result<Option<PathBuf>> {
//             let config = self.config()?;
//             let Some(book_config_path) = config.voice_script_config() else {
//                 return Ok(None);
//             };
//             Ok(Some(book_config_path.to_std_path(self.root())))
//         }

//         #[result]
//         pub(crate) fn book_config(&self) -> anyhow::Result<Option<BookConfig>> {
//             let Some(book_config_path) = self.book_config_path()? else {
//                 return Ok(None);
//             };
//             let script_config: BookConfig = load_config(book_config_path, &ConfigFormat::Toml)?;
//             Ok::<_, anyhow::Error>(Some(script_config))
//         }

//         #[result]
//         pub(crate) fn book_opt(&self) -> anyhow::Result<Option<Book>> {
//             let Some(resource_set) = self.resources_opt()? else {
//                 return Ok(None);
//             };

//             let Some(book_config) = self.book_config()? else {
//                 return Ok(None);
//             };

//             let mut builder = BookBuilder::new(book_config.clone())?;

//             for res in resource_set.resources_of_type(ResourceType::Message) {
//                 let msg_resources = parse_message_resource(&res.data().open_mem(..)?)?;
//                 for (msg_id, record) in msg_resources.messages() {
//                     builder.add_message(res.id().resource_num(), msg_id, record)?;
//                 }
//             }

//             Ok(Some(builder.build()?))
//         }

//         #[result]
//         pub(crate) fn scripts_dir_opt(&self) -> anyhow::Result<Option<AbsPathBuf>> {
//             let scripts_rel_path = self
//                 .config()?
//                 .source_config()
//                 .source_dir()
//                 .unwrap_or(RelPath::new(SCRIPTS_DEFAULT_PATH));
//             let possible_scripts_dir = self.root().join(scripts_rel_path);
//             if possible_scripts_dir.is_dir() {
//                 Ok::<_, anyhow::Error>(Some(AbsPathBuf::from_std(possible_scripts_dir)))
//             } else {
//                 Ok(None)
//             }
//         }
//     }
// }

// pub(crate) use project_impl::Project;

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

    pub(crate) fn generate_from_game_dir(source: impl AsRef<Path>) -> anyhow::Result<Self> {
        let source = source.as_ref();
        let walker = RelWalkDir::new(source);
        let mut manifest = Manifest::new();
        for path in walker {
            let path = path?;
            let file = std::fs::File::open(source.join(&path))?;
            manifest.add_from_reader(path, file)?;
        }
        Ok(manifest)
    }
}
