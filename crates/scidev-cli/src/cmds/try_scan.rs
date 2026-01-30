use std::path::Path;

pub(crate) fn try_scan(scan_dir: &Path) -> anyhow::Result<()> {
    let scan = sciproj::file::AudioSampleScan::read_from_dir(scan_dir)?;

    anyhow::ensure!(
        !scan.has_duplicates(),
        "Duplicate files found in scan directory",
    );

    eprintln!("Scan directory: {}", scan.base_path().display());
    for (line_id, sample) in scan.get_valid_entries() {
        eprintln!("{line_id}: {sample:?}");
    }
    Ok(())
}
