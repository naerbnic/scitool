use std::path::Path;

pub(crate) async fn export_scannable(sample_dir: &Path, output_dir: &Path) -> anyhow::Result<()> {
    let sample_dir = sciproj::resources::SampleDir::load_dir(sample_dir)?;
    sample_dir.save_to_scannable_dir(output_dir).await?;
    Ok(())
}
