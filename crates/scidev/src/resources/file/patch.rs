use std::{ffi::OsStr, path::Path};

use crate::utils::block::BlockSource;

use crate::resources::{ResourceId, ResourceType};
use crate::utils::errors::ensure_other;
use crate::utils::errors::{OtherError, prelude::*};

use super::Resource;

#[derive(thiserror::Error, Debug)]
pub(crate) enum TryPatchError {
    #[doc(hidden)]
    #[error(transparent)]
    Other(#[from] OtherError),
}

pub(crate) fn try_patch_from_file(patch_file: &Path) -> Result<Option<Resource>, TryPatchError> {
    // Parse the filename to get the resource ID.

    // The stem of the file is the resource ID as an integer.
    let Some(stem) = patch_file.file_stem().and_then(OsStr::to_str) else {
        return Ok(None);
    };

    let Some(ext) = patch_file.extension().and_then(OsStr::to_str) else {
        return Ok(None);
    };

    let res_num: u16 = if let Ok(res_num) = str::parse(stem) {
        res_num
    } else {
        return Ok(None);
    };

    let Ok(res_type) = ResourceType::from_file_ext(ext) else {
        return Ok(None);
    };

    let source = BlockSource::from_path(patch_file.to_path_buf()).with_other_err()?;
    let (base_header_block, rest) = source.split_at(2);
    let base_header = base_header_block.open().with_other_err()?;
    let id = base_header[0];
    let header_size = base_header[1];
    let content_res_type: ResourceType = id.try_into().with_other_err()?;
    ensure_other!(
        content_res_type == res_type,
        "Resource type mismatch: expected {:?}, got {:?}",
        res_type,
        content_res_type
    );

    // Looking at the ScummVM source code, it
    // doesn't appear that the data is used during execution, so we can skip
    // over it.
    //
    // It looks like there's a fairly simple scheme. If the byte after the type is
    // 128, then we use an extended header, including a two-byte length field,
    // and another 22 byte header data that we can skip.
    let data = if header_size == 128 {
        let (header_data, rest) = rest.split_at(24);
        let header_data = header_data.open().with_other_err()?;
        let real_header_size = header_data[1];
        if real_header_size != 0 {
            log::warn!(
                "Patch file header size is not 0, got (size {real_header_size}) {header_data:?} ({}, {res_type:?})",
                patch_file.display()
            );
        }
        rest.subblock(u64::from(real_header_size)..).to_lazy_block()
    } else {
        rest.subblock(u64::from(header_size)..).to_lazy_block()
    };

    Ok(Some(Resource {
        id: ResourceId::new(res_type, res_num),
        source: data,
    }))
}
