use std::path::Path;

use sciproj::{
    build::audio::compile_audio_base,
    resources::{ScanType, load_config_from_directory},
};

pub(crate) async fn compile_audio(
    scan_type: ScanType,
    sample_dir: &Path,
    output_dir: &Path,
) -> anyhow::Result<()> {
    let sample_set = load_config_from_directory(scan_type, sample_dir)?;

    compile_audio_base(&sample_set, output_dir).await?;
    Ok(())
}
