#![cfg_attr(not(windows), no_main)]

#[cfg(not(windows))]
use scidev::resources::ResourceSet;

#[cfg(not(windows))]
fn body(root_dir: &std::path::Path) -> anyhow::Result<()> {
    let _resources = ResourceSet::from_root_dir(root_dir)?;
    Ok(())
}

#[cfg(not(windows))]
libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    if data.len() < 8 {
        return;
    }
    let (map_file_size_bytes, file_data) = data.split_at(8);
    let map_file_size = u64::from_le_bytes(map_file_size_bytes.try_into().unwrap());
    if file_data.len() < map_file_size as usize {
        return;
    }

    let (map_file_data, resource_file_data) = file_data.split_at(map_file_size as usize);
    let tempdir = tempfile::tempdir().unwrap();

    std::fs::write(tempdir.path().join("RESOURCE.MAP"), map_file_data).unwrap();
    std::fs::write(tempdir.path().join("RESOURCE.000"), resource_file_data).unwrap();

    let _ = body(tempdir.path());
});

#[cfg(windows)]
fn main() {
    eprintln!("Fuzz target is only available on non-windows platforms.")
}