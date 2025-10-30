use anyhow::anyhow;
use scidev::{
    resources::{ResourceId, ResourceSet, ResourceType},
    utils::debug::hex_dump_to,
};
use std::path::Path;

use crate::respack::ResPack;

pub fn dump_resource(
    root_dir: &Path,
    resource_id: ResourceId,
    output: impl std::io::Write,
) -> anyhow::Result<()> {
    let resource_set = ResourceSet::from_root_dir(root_dir)?;
    let res = resource_set
        .get_resource(&resource_id)
        .ok_or_else(|| anyhow::anyhow!("Resource not found: {resource_id:?}"))?;
    let data = res.data().open_mem(..)?;
    hex_dump_to(output, &data, 0)?;
    Ok(())
}

pub struct WriteOperation<'a> {
    pub resource_id: ResourceId,
    pub filename: String,
    pub operation: Box<dyn FnOnce() -> anyhow::Result<()> + 'a>,
}

pub fn extract_resource_as_patch<'a>(
    root_dir: &'a Path,
    resource_type: ResourceType,
    resource_num: u16,
    output_dir: &'a Path,
) -> anyhow::Result<WriteOperation<'a>> {
    let resource_set = ResourceSet::from_root_dir(root_dir)?;
    let resource_id = ResourceId::new(resource_type, resource_num);
    let contents = resource_set
        .get_resource(&resource_id)
        .ok_or_else(|| anyhow::anyhow!("Resource not found: {resource_id:?}"))?;
    let ext = match resource_type {
        ResourceType::Script => "SCR",
        ResourceType::Heap => "HEP",
        _ => {
            anyhow::bail!("Unsupported resource type");
        }
    };

    let filename = output_dir.join(format!("{0}.{1}", resource_id.resource_num(), ext));

    eprintln!(
        "Writing resource {restype:?}:{resid} to {filename}",
        restype = resource_type,
        resid = resource_num,
        filename = filename.display()
    );
    let operation = Box::new({
        let filename = filename.clone();
        move || {
            let mut patch_file = std::fs::File::options()
                .write(true)
                .create_new(true)
                .open(root_dir.join(filename))?;
            contents.write_patch(&mut patch_file)?;

            Ok(())
        }
    });

    Ok(WriteOperation {
        resource_id,
        filename: filename.display().to_string(),
        operation,
    })
}

pub fn list_resources(
    root_dir: &Path,
    res_type: Option<ResourceType>,
) -> anyhow::Result<Vec<ResourceId>> {
    let resource_dir_files = ResourceSet::from_root_dir(root_dir)?;
    let resources: Vec<ResourceId> = resource_dir_files
        .resources()
        .map(|r| *r.id())
        .filter(|id| res_type.is_none_or(|res_type| id.type_id() == res_type))
        .collect();
    Ok(resources)
}

pub fn export(root_dir: &Path, resource_id: ResourceId, output_path: &Path) -> anyhow::Result<()> {
    let resource_set = ResourceSet::from_root_dir(root_dir)?;
    let mut respack = ResPack::from_resource(
        &resource_set
            .get_resource(&resource_id)
            .ok_or_else(|| anyhow!("Resource not found: {resource_id:?}"))?,
    )?;

    respack.save_to(output_path)?;

    Ok(())
}

pub fn export_all(root_dir: &Path, output_root: &Path) -> anyhow::Result<()> {
    let resource_set = ResourceSet::from_root_dir(root_dir)?;
    for resource in resource_set.resources() {
        let mut respack = ResPack::from_resource(&resource)?;
        let output_path = output_root.join(format!(
            "{}.{:03}.respack",
            resource.id().resource_num(),
            resource.id().type_id().to_file_ext(),
        ));
        eprintln!(
            "Exporting resource {:?} to {}",
            resource.id(),
            output_path.display()
        );
        respack.save_to(&output_path)?;
    }

    Ok(())
}
