use anyhow::Context as _;
use scidev_inc::{ArtifactKey, FnExt as _, Operation};

#[derive(Hash, Eq, PartialEq, Debug)]
pub(super) struct ProjectConfig;

impl ArtifactKey for ProjectConfig {
    type Value = anyhow::Result<crate::commands::config::ProjectConfig>;
}

impl Operation for ProjectConfig {
    fn run(&self, ctx: &scidev_inc::TaskContext<'_>) -> Self::Value {
        let config_path = ctx
            .get_input(super::paths::config_path_fn.to_op())
            .can_fail_disp()?;
        let contents = std::fs::read(&*config_path).context(anyhow::format_err!(
            "Unable to open project file {config_path}"
        ))?;
        let config: crate::project::ProjectConfig = toml::from_slice(&contents)
            .context(format!("Unable to parse project file {config_path}"))?;
        Ok(config)
    }
}
