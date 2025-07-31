use std::path::Path;

use scidev_resources::{ResourceId, ResourceType, file::open_game_resources};
use scidev_utils::data_writer::{DataWriter, IoDataWriter};

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
    let resource_set = open_game_resources(root_dir)?;
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
        move || {
            let mut patch_file = IoDataWriter::new(
                std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(root_dir.join(filename))?,
            );

            patch_file.write_u8(resource_type.into())?;
            patch_file.write_u8(0)?; // Header Size
            patch_file.write_block(&contents.load_data()?)?;
            Ok(())
        }
    });

    Ok(WriteOperation {
        resource_id,
        filename: filename.display().to_string(),
        operation,
    })
}
