#![no_main]

use libfuzzer_sys::fuzz_target;
use scidev::utils::compression::dcl::{
    CompressionMode, DictType, compress_reader, decompress_reader,
};

fuzz_target!(|data: &[u8]| {
    let mut output = Vec::new();
    let mut reader = decompress_reader(compress_reader(
        CompressionMode::Binary,
        DictType::Size1024,
        data,
    ));

    std::io::copy(&mut reader, &mut output).unwrap();

    assert_eq!(data, &*output);
});
