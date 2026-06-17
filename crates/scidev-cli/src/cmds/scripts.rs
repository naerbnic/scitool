use std::{
    io::{BufWriter, Write},
    path::Path,
};

use scidev::resources::ResourceSet;
use sciproj::scripts::SciScriptExports;

pub(crate) fn generate_headers(
    game_dir: &Path,
    selectors_path: &Path,
    classdef_path: &Path,
) -> anyhow::Result<()> {
    let resource_set = ResourceSet::from_root_dir(game_dir)?;
    let exports = SciScriptExports::read_from_resources(&resource_set)?;

    std::fs::write(selectors_path, exports.selectors_header())?;
    std::fs::write(classdef_path, exports.class_defs_header())?;

    Ok(())
}

pub(crate) fn dump_headers(game_dir: &Path, mut output: impl std::io::Write) -> anyhow::Result<()> {
    let resource_set = ResourceSet::from_root_dir(game_dir)?;
    let exports = SciScriptExports::read_from_resources(&resource_set)?;

    let mut output = BufWriter::new(&mut output);

    writeln!(output, "Selectors:")?;
    output.write_all(exports.selectors_header().as_bytes())?;

    writeln!(output, "Class Definitions:")?;
    output.write_all(exports.class_defs_header().as_bytes())?;

    Ok(())
}
