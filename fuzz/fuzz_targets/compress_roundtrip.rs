#![cfg_attr(not(windows), no_main)]

#[cfg(not(windows))]
use scidev::utils::compression::dcl::{
    CompressionMode, DictType, compress_reader, decompress_reader,
};

#[cfg(not(windows))]
libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    
    let mut output = Vec::new();
    let mut reader = decompress_reader(compress_reader(
        CompressionMode::Binary,
        DictType::Size1024,
        data,
    ));

    std::io::copy(&mut reader, &mut output).unwrap();

    assert_eq!(data, &*output);
});

#[cfg(windows)]
fn main() {
    eprintln!("Fuzz target is only available on non-windows platforms.")
}