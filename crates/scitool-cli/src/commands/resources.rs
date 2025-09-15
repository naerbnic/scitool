use scidev::{
    resources::{ExtraData, ResourceId, ResourceSet, ResourceType},
    utils::debug::hex_dump_to,
};
use std::path::Path;

pub fn dump_resource(
    root_dir: &Path,
    resource_id: ResourceId,
    output: impl std::io::Write,
) -> anyhow::Result<()> {
    let resource_set = ResourceSet::from_root_dir(root_dir)?;
    let res = resource_set
        .get_resource(&resource_id)
        .ok_or_else(|| anyhow::anyhow!("Resource not found: {:?}", resource_id))?;
    let data = res.load_data()?;
    hex_dump_to(output, &data, 0)?;
    Ok(())
}

pub struct WriteOperation<'a> {
    pub resource_id: ResourceId,
    pub filename: String,
    pub operation: Box<dyn Future<Output = anyhow::Result<()>> + 'a>,
}

pub async fn extract_resource_as_patch<'a>(
    root_dir: &'a Path,
    resource_type: ResourceType,
    resource_num: u16,
    output_dir: &'a Path,
) -> anyhow::Result<WriteOperation<'a>> {
    let resource_set = ResourceSet::from_root_dir(root_dir)?;
    let resource_id = ResourceId::new(resource_type, resource_num);
    let contents = resource_set
        .get_resource(&resource_id)
        .ok_or_else(|| anyhow::anyhow!("Resource not found: {:?}", resource_id))?;
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
        async move {
            let mut patch_file = tokio::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(root_dir.join(filename))
                .await?;
            contents.write_patch(&mut patch_file).await?;

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
    Ok(resource_dir_files
        .resources()
        .inspect(|r| {
            if let Some(extra_data) = r.extra_data() {
                match extra_data {
                    ExtraData::Simple(data) => {
                        let data = data.open().unwrap();
                        if !data.is_empty() {
                            eprintln!("Found resource with extra data: {:?}, {:?}", r.id(), data);
                        }
                    }
                    ExtraData::Composite {
                        ext_header,
                        extra_data,
                    } => {
                        let ext_data = ext_header.open().unwrap();
                        if !ext_data.is_empty() {
                            eprintln!(
                                "Found resource with extended header: {:?}, {:?}",
                                r.id(),
                                ext_data
                            );
                        }
                        let data = extra_data.open().unwrap();
                        if !data.is_empty() {
                            eprintln!("Found resource with extra data: {:?}, {:?}", r.id(), data);
                        }
                    }
                }
            }
        })
        .map(|r| *r.id())
        .filter(|id| res_type.is_none_or(|res_type| id.type_id() == res_type))
        .collect())
}
