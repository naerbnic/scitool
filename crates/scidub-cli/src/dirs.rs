//! Functions to acquire the directories that are needed to load tool configuration,
//! find distributed binaries, or load/store user-specific data.

use std::path::{Path, PathBuf};

use fs_mistrust::Mistrust;
use sciproj::{
    path::{LookupPath, is_executable},
    tools::Tool,
};

cfg_select! {
    windows => {
        // The expected layout of a Windows distribution:
        //
        // scidub/
        //   scidub.exe
        //   config.toml  // Global config. If missing, use defaults.
        //   tools/
        //     ffmpeg.exe
        //     espeak-ng.exe
        //     scinc.exe
        //     ...
        //   share/
        //     espeak-ng/
        //       ...
        //     scinc/
        //       ...

        // Root is same directory as the executable.
        const EXE_TO_ROOT_PATH: &str = ".";
        const BIN_PATH: &str = "scidub.exe";
        const CONFIG_PATH: &str = "config.toml";
        const EXTERNAL_BIN_PATH: &str = "tools";
        const EXTERNAL_DATA_PATH: &str = "share";
        const EXE_EXT: Option<&str> = Some(".exe");
    }

    unix => {
        // The expected layout of a Unix distribution:
        //
        // scidub/
        //   bin/
        //     scidub
        //   etc/
        //     scidub.toml
        //   libexec/
        //     ffmpeg
        //     espeak-ng
        //     scinc
        //   share/
        //     espeak-ng/
        //       ...
        //     scinc/
        //       ...

        // macOS and Linux follow standard FHS general approaches, where the
        // binary is in a "bin/" directory, and external tool dependencies
        // are in "libexec/scidub/"

        // Root is the parent directory of the executable.
        const EXE_TO_ROOT_PATH: &str = "..";
        const BIN_PATH: &str = "bin/scidub";
        const CONFIG_PATH: &str = "etc/scidub.toml";
        const EXTERNAL_BIN_PATH: &str = "libexec";
        const EXTERNAL_DATA_PATH: &str = "share";
        const EXE_EXT: Option<&str> = None;
    }
}

fn get_current_exe_root() -> anyhow::Result<Option<PathBuf>> {
    // The defaults are to find a directory relative to the executable location.
    let exe_path = std::env::current_exe()?.canonicalize()?;
    if !exe_path.ends_with(BIN_PATH) {
        return Ok(None);
    }
    let root = exe_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Can't find directory of executable."))?
        .join(EXE_TO_ROOT_PATH);
    Ok(Some(root))
}

#[derive(Debug, Clone)]
struct DefaultPaths {
    _root: PathBuf,
    _config_path: PathBuf,
    ext_bin_dir: PathBuf,
    ext_data_dir: PathBuf,
}

impl DefaultPaths {
    fn from_root(root: &Path) -> anyhow::Result<Option<Self>> {
        let mistrust = Mistrust::new();
        let config_path = root.join(CONFIG_PATH);
        let ext_bin_dir = root.join(EXTERNAL_BIN_PATH);
        let ext_data_dir = root.join(EXTERNAL_DATA_PATH);

        if !ext_bin_dir.is_dir() {
            return Ok(None);
        }

        if !ext_data_dir.is_dir() {
            return Ok(None);
        }

        mistrust.check_directory(root)?;
        mistrust.check_directory(&ext_bin_dir)?;
        mistrust.check_directory(&ext_data_dir)?;
        Ok(Some(DefaultPaths {
            _root: root.to_path_buf(),
            _config_path: config_path,
            ext_bin_dir,
            ext_data_dir,
        }))
    }
}

fn add_exec_extension(mut path: PathBuf) -> PathBuf {
    if let Some(ext) = EXE_EXT {
        path.add_extension(ext);
    }
    path
}

pub(crate) struct DistEnvBuilder {
    use_system_path: bool,
}

impl DistEnvBuilder {
    pub(crate) fn new() -> Self {
        Self {
            use_system_path: false,
        }
    }

    pub(crate) fn set_use_system_path(mut self, use_system_path: bool) -> Self {
        self.use_system_path = use_system_path;
        self
    }

    pub(crate) fn build_from_current_exe(self) -> anyhow::Result<DistEnv> {
        Self::build_from_root(self, get_current_exe_root()?.as_deref())
    }

    fn build_from_root(self, root: Option<&Path>) -> anyhow::Result<DistEnv> {
        let paths = if let Some(root) = root {
            DefaultPaths::from_root(root)?
        } else {
            None
        };
        Ok(DistEnv {
            sys_path: if self.use_system_path {
                Some(LookupPath::from_env())
            } else {
                None
            },
            paths,
        })
    }
}

struct ExternalToolPaths {
    bin: PathBuf,
    data: Option<PathBuf>,
}

pub(crate) struct DistEnv {
    sys_path: Option<LookupPath>,
    paths: Option<DefaultPaths>,
}

impl DistEnv {
    pub(crate) fn builder() -> DistEnvBuilder {
        DistEnvBuilder::new()
    }

    fn find_binary(&self, name: &str) -> Option<ExternalToolPaths> {
        let name_path: &Path = name.as_ref();
        let bin_filename = add_exec_extension(name_path.to_path_buf());

        if let Some(paths) = &self.paths {
            let possible_ext_bin_path = paths.ext_bin_dir.join(&bin_filename);
            if is_executable(&possible_ext_bin_path) {
                return Some(ExternalToolPaths {
                    bin: possible_ext_bin_path,
                    data: self.find_data_dir(name),
                });
            }
        }

        if let Some(sys_path) = &self.sys_path
            && let Some(bin_path) = sys_path.find_binary(name_path)
        {
            return Some(ExternalToolPaths {
                bin: bin_path.to_path_buf(),
                data: None,
            });
        }
        None
    }

    fn find_data_dir(&self, name: &str) -> Option<PathBuf> {
        if let Some(paths) = &self.paths {
            let possible_ext_bin_path = paths.ext_data_dir.join(name);
            if is_executable(&possible_ext_bin_path) {
                return Some(possible_ext_bin_path);
            }
        }
        None
    }

    pub(crate) fn ffmpeg_tool(&self) -> Tool {
        Tool::from_path(
            self.find_binary("ffmpeg")
                .expect("Unable to find ffmpeg")
                .bin,
        )
    }

    pub(crate) fn espeak_tool(&self) -> Option<Tool> {
        let tool_env = self.find_binary("espeak")?;
        let mut espeak = Tool::from_path(tool_env.bin);
        if let Some(data) = tool_env.data {
            espeak = espeak.with_env("ESPEAK_DATA_PATH", data.to_str().unwrap());
        }
        Some(espeak)
    }
}
