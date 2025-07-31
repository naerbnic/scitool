use std::path::Path;

use scidev_resources::{ResourceId, ResourceType, file::open_game_resources};

pub fn list_resources(
    root_dir: &Path,
    res_type: Option<ResourceType>,
) -> anyhow::Result<Vec<ResourceId>> {
    let resource_dir_files = open_game_resources(root_dir)?;
    Ok(resource_dir_files
        .resource_ids()
        .filter(|id| res_type.is_none_or(|res_type| id.type_id() == res_type))
        .collect())
}
