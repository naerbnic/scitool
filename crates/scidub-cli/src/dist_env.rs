//! Functions to acquire the directories that are needed to load tool configuration,
//! find distributed binaries, or load/store user-specific data.

use std::{collections::BTreeMap, sync::LazyLock};

use fs_mistrust::Mistrust;
use sciproj::{
    path::{
        LookupPath,
        abspath::{AbsPath, AbsPathBuf},
        is_executable,
        relpath::{RelPath, RelPathBuf},
    },
    tools::{
        espeak::{self, BoxEspeak},
        ffmpeg::{self, BoxFfmpeg},
        location::{DistLocation, SystemPathLocation, ToolLocation},
        scinc::{self, BoxScinc},
    },
};
use tracing::{info, instrument};

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
        const EXE_TO_ROOT_PATH: &str = "..";
        const BIN_PATH: &str = "bin/scidub.exe";
        const CONFIG_PATH: &str = "config.toml";
        const EXTERNAL_ROOT_PATH: &str = "libexec/scidub";
        const EXTERNAL_BIN_PATH: &str = "bin";
        const EXE_EXT: Option<&str> = Some("exe");
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
        const EXTERNAL_ROOT_PATH: &str = "libexec/scidub";
        const EXTERNAL_BIN_PATH: &str = "bin";
        const EXE_EXT: Option<&str> = None;
    }
}

#[instrument]
fn get_current_exe_root() -> anyhow::Result<Option<AbsPathBuf>> {
    // The defaults are to find a directory relative to the executable location.
    let exe_path = std::env::current_exe()?.canonicalize()?;
    tracing::info!("exe_path: {}", exe_path.display());
    if !exe_path.ends_with(BIN_PATH) {
        tracing::info!("Invalid: does not end with BIN_PATH = {:?}", BIN_PATH);
        return Ok(None);
    }
    let root = exe_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Can't find directory of executable."))?
        .join(EXE_TO_ROOT_PATH);

    // Recanonicalize, to make sure there are no intermediate ".." segments.
    let root = root.canonicalize()?;
    let Some(abs_root) = AbsPathBuf::try_from_std(root.as_path()) else {
        anyhow::bail!("Invalid abs_root")
    };
    info!("Found valid exe root: {}", abs_root);
    Ok(Some(abs_root))
}

#[derive(Debug, Clone)]
struct DefaultPaths {
    _root: AbsPathBuf,
    _config_path: AbsPathBuf,
    ext_root_dir: AbsPathBuf,
    ext_bin_dir: RelPathBuf,
}

impl DefaultPaths {
    fn from_root(root: &AbsPath) -> anyhow::Result<Option<Self>> {
        let mistrust = Mistrust::new();
        let config_path = root.join(CONFIG_PATH);
        let ext_root_dir = root.join(EXTERNAL_ROOT_PATH);
        let ext_bin_dir = ext_root_dir.join(EXTERNAL_BIN_PATH);

        info!(
            "Checking paths: ext_bin_dir: {}, ext_data_dir: {}",
            ext_root_dir, ext_bin_dir
        );

        if !ext_bin_dir.is_dir() {
            return Ok(None);
        }

        if !ext_root_dir.is_dir() {
            return Ok(None);
        }

        let verifier = mistrust.verifier().permit_readable().require_directory();

        verifier.check(root)?;
        verifier.check(&ext_bin_dir)?;
        verifier.check(&ext_root_dir)?;

        info!("Checks succeeded");
        Ok(Some(DefaultPaths {
            _root: root.to_buf(),
            _config_path: config_path,
            ext_root_dir,
            ext_bin_dir: RelPathBuf::new(EXTERNAL_BIN_PATH),
        }))
    }
}

fn add_exec_extension(mut path: RelPathBuf) -> RelPathBuf {
    if let Some(ext) = EXE_EXT {
        path.add_extension(ext);
    }
    path
}

/// Non-default env vars that are used in the configuration of the distribution
/// environment.
static CAPTURED_ENV_VARS: &[&str] = &["SCINC_HOME"];

#[derive(Debug)]
pub(crate) struct DistEnvBuilder {
    use_system_path: bool,
    env_vars: BTreeMap<String, String>,
}

impl DistEnvBuilder {
    pub(crate) fn new() -> Self {
        Self {
            use_system_path: false,
            env_vars: BTreeMap::new(),
        }
    }

    pub(crate) fn set_use_system_path(mut self, use_system_path: bool) -> Self {
        self.use_system_path = use_system_path;
        self
    }

    fn add_env_var(mut self, var: &str, value: &str) -> anyhow::Result<Self> {
        let old_value = self.env_vars.insert(var.to_string(), value.to_string());
        anyhow::ensure!(old_value.is_none(), "Tried to set {var} twice");
        Ok(self)
    }

    #[instrument]
    pub(crate) fn build_from_current_exe(self) -> anyhow::Result<DistEnv> {
        Self::build_from_root(self, get_current_exe_root()?.as_deref())
    }

    fn build_from_root(self, root: Option<&AbsPath>) -> anyhow::Result<DistEnv> {
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
            env_vars: self.env_vars,
        })
    }
}

static DIST_ENV: LazyLock<DistEnv> =
    LazyLock::new(|| DistEnv::try_load_env().expect("Unable to build distribution environment"));

#[derive(Debug)]
pub(crate) struct DistEnv {
    sys_path: Option<LookupPath>,
    paths: Option<DefaultPaths>,
    env_vars: BTreeMap<String, String>,
}

impl DistEnv {
    pub(crate) fn try_load_env() -> anyhow::Result<Self> {
        let mut builder = DistEnvBuilder::new()
            .set_use_system_path(cfg!(feature = "search_system_path_for_tools"));

        if cfg!(feature = "use_env_vars_for_tools") {
            for var in CAPTURED_ENV_VARS {
                match std::env::var(var) {
                    Ok(value) => builder = builder.add_env_var(var, &value)?,
                    Err(std::env::VarError::NotPresent) => {}
                    Err(std::env::VarError::NotUnicode(err)) => {
                        anyhow::bail!("Unexpected non-unicode value: {}", err.display())
                    }
                }
            }
        }

        builder.build_from_current_exe()
    }

    pub(crate) fn from_env() -> &'static Self {
        &DIST_ENV
    }

    #[instrument]
    fn find_binary(&self, name: &str) -> Vec<ToolLocation> {
        let mut found_locations = Vec::new();
        let name_path: &RelPath = RelPath::new(name);
        let bin_filename = add_exec_extension(name_path.to_buf());

        info!("Searching for {}", bin_filename);

        if let Some(paths) = &self.paths {
            let possible_ext_bin_path = paths
                .ext_root_dir
                .join_rel(&paths.ext_bin_dir)
                .join(&bin_filename);
            info!(
                "Searching for {} in {:?}",
                bin_filename, possible_ext_bin_path
            );
            if is_executable(&possible_ext_bin_path) {
                found_locations.push(
                    DistLocation::new(
                        paths.ext_root_dir.clone(),
                        paths.ext_bin_dir.join(&bin_filename),
                    )
                    .into(),
                );
            } else {
                info!("{} not found", bin_filename);
            }
        }

        if let Some(sys_path) = &self.sys_path {
            info!(
                "Searching for {} in system path {:?}",
                bin_filename, sys_path
            );
            if let Some(bin_path) = sys_path.find_binary(name_path) {
                info!("Found {} in system path", bin_filename);
                found_locations
                    .push(SystemPathLocation::new(AbsPathBuf::from_std(bin_path)).into());
            }
        }
        found_locations
    }

    pub(crate) fn ffmpeg_tool(&self) -> anyhow::Result<Option<BoxFfmpeg>> {
        let Some(ffmpeg_loc) = self.find_binary("ffmpeg").first().cloned() else {
            return Ok(None);
        };
        Ok(Some(ffmpeg::from_tool_location(&ffmpeg_loc)?))
    }

    pub(crate) fn espeak_tool(&self) -> anyhow::Result<Option<BoxEspeak>> {
        let Some(first_loc) = self.find_binary("espeak-ng").first().cloned() else {
            return Ok(None);
        };
        Ok(Some(espeak::from_tool_location(&first_loc)?))
    }

    pub(crate) fn get_env_var(&self, var: &str) -> Option<&str> {
        if let Some(value) = self.env_vars.get(var) {
            Some(value)
        } else {
            None
        }
    }

    pub(crate) fn scinc_tool(&self) -> anyhow::Result<Option<BoxScinc>> {
        // Look at the SCINC_HOME env var. This is mostly used for local testing.
        //
        // The structure of the directory is expected to be the same as the produced scinc.tgz file
        // in its repo.
        if let Some(home) = self.get_env_var("SCINC_HOME") {
            let home_path = AbsPath::new_opt(home)
                .ok_or_else(|| anyhow::anyhow!("Invalid SCINC_HOME: {home}"))?;
            let bin_path = home_path.join(format!("bin/scinc{}", std::env::consts::EXE_SUFFIX));
            let include_path = home_path.join("include");
            anyhow::ensure!(
                bin_path.is_file(),
                "Expected scinc binary does not exist. Path: {bin_path}, Home: {home}",
            );
            anyhow::ensure!(
                include_path.is_dir(),
                "Expected scinc include directory does not exist. Path: {include_path}, Home: {home}"
            );
            return Ok(Some(scinc::from_binary_include(bin_path, include_path)?));
        }

        let Some(dist_loc) = self.find_binary("scinc").into_iter().find_map(|loc| {
            if let ToolLocation::Dist(dist) = loc {
                Some(dist)
            } else {
                None
            }
        }) else {
            return Ok(None);
        };
        Ok(Some(scinc::from_dist_location(&dist_loc)?))
    }
}
