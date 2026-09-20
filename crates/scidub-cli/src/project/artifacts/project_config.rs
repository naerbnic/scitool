use std::path::PathBuf;

use anyhow::Context as _;
use scidev::resources::{ResourceSet, ResourceType, types::msg::parse_message_resource};
use scidev_inc::TaskContext;
use sciproj::{
    book::{Book, builder::BookBuilder, config::BookConfig},
    path::{abspath::AbsPathBuf, relpath::RelPath},
};

use crate::{
    data::{ConfigFormat, load_config},
    project::{SCRIPTS_DEFAULT_PATH, artifacts::root},
};

pub(super) fn project_config(
    ctx: &TaskContext<'_>,
) -> anyhow::Result<crate::project::ProjectConfig> {
    let config_path = ctx
        .op(super::paths::config_path)
        .can_fail_disp()
        .get_sync()?;
    let contents = std::fs::read(config_path).context(anyhow::format_err!(
        "Unable to open project file {config_path}"
    ))?;
    let config: crate::project::ProjectConfig = toml::from_slice(&contents)
        .context(format!("Unable to parse project file {config_path}"))?;
    Ok(config)
}

pub(super) fn build_dir(ctx: &TaskContext<'_>) -> anyhow::Result<AbsPathBuf> {
    let project_config = ctx.cached_disp(project_config)?;
    let build_dir = project_config.build_dir().unwrap_or(RelPath::new("build"));
    Ok(ctx.get_input(super::root::Root).join_rel(build_dir))
}

pub(crate) fn game_path_opt(ctx: &TaskContext<'_>) -> anyhow::Result<Option<AbsPathBuf>> {
    let project_config = ctx.cached_disp(project_config)?;
    let Some(game_path) = project_config.game_files() else {
        return Ok(None);
    };
    Ok(Some(ctx.get_input(super::root::Root).join(game_path)))
}

pub(crate) fn resources_opt(ctx: &TaskContext<'_>) -> anyhow::Result<Option<ResourceSet>> {
    let result = ctx.cached_disp(game_path_opt)?;
    let Some(game_path) = result.as_ref() else {
        return Ok(None);
    };

    let resource_set = ResourceSet::from_root_dir(&game_path.to_std())?;
    Ok(Some(resource_set))
}

pub(crate) fn book_config_path(ctx: &TaskContext<'_>) -> anyhow::Result<Option<PathBuf>> {
    let config = ctx.op(project_config).can_fail_disp().get_sync()?;
    let Some(book_config_path) = config.voice_script_config() else {
        return Ok(None);
    };
    Ok(Some(
        book_config_path.to_std_path(ctx.get_input(root::Root).as_path()),
    ))
}

pub(crate) fn book_config(ctx: &TaskContext<'_>) -> anyhow::Result<Option<BookConfig>> {
    let book_path = ctx.cached_disp(book_config_path)?;
    let Some(book_config_path) = book_path.as_ref() else {
        return Ok(None);
    };
    let book_config: BookConfig = load_config(book_config_path, &ConfigFormat::Toml)?;
    Ok::<_, anyhow::Error>(Some(book_config))
}

pub(crate) fn book_opt(ctx: &TaskContext<'_>) -> anyhow::Result<Option<Book>> {
    let resources_opt = ctx.op(resources_opt).can_fail_disp().get_sync()?;
    let Some(resource_set) = resources_opt.as_ref() else {
        return Ok(None);
    };

    let book_config_opt = ctx.op(book_config).can_fail_disp().get_sync()?;
    let Some(book_config) = book_config_opt.as_ref() else {
        return Ok(None);
    };

    let mut builder = BookBuilder::new(book_config.clone())?;

    for res in resource_set.resources_of_type(ResourceType::Message) {
        let msg_resources = parse_message_resource(&res.data().open_mem(..)?)?;
        for (msg_id, record) in msg_resources.messages() {
            builder.add_message(res.id().resource_num(), msg_id, record)?;
        }
    }

    Ok(Some(builder.build()?))
}

pub(crate) fn scripts_dir_opt(ctx: &TaskContext<'_>) -> anyhow::Result<Option<AbsPathBuf>> {
    let project_config = ctx.op(project_config).can_fail_disp().get_sync()?;
    let scripts_rel_path = project_config
        .source_config()
        .source_dir()
        .unwrap_or(RelPath::new(SCRIPTS_DEFAULT_PATH));
    let possible_scripts_dir = ctx.get_input(root::Root).join(scripts_rel_path);
    if possible_scripts_dir.is_dir() {
        Ok::<_, anyhow::Error>(Some(AbsPathBuf::from_std(possible_scripts_dir)))
    } else {
        Ok(None)
    }
}
