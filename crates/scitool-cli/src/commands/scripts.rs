use std::{fs::File, io::BufWriter, path::Path};

use self::headers::SciScriptExports;
mod headers;

pub fn generate_headers(
    game_dir: &Path,
    selectors_path: &Path,
    classdef_path: &Path,
) -> anyhow::Result<()> {
    let exports = SciScriptExports::read_from_resources(game_dir)?;

    exports.write_selector_header_to(BufWriter::new(File::create(selectors_path)?))?;
    exports.write_classdef_header_to(BufWriter::new(File::create(classdef_path)?))?;

    Ok(())
}
