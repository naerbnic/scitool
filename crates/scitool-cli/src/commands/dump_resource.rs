use std::path::Path;

use scidev_resources::{ResourceId, file::open_game_resources};

pub fn dump_resource(root_dir: &Path, resource_id: ResourceId) -> anyhow::Result<String> {
    let resource_set = open_game_resources(root_dir)?;
    let res = resource_set
        .get_resource(&resource_id)
        .ok_or_else(|| anyhow::anyhow!("Resource not found: {:?}", resource_id))?;
    let data = res.load_data()?;
    let mut dump = Vec::new();
    scidev_utils::debug::hex_dump_to(std::io::Cursor::new(&mut dump), &data, 0)?;
    Ok(String::from_utf8_lossy(&dump).into_owned())
}
