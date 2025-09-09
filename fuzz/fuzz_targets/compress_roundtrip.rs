#![no_main]

use libfuzzer_sys::fuzz_target;
use scidev::utils::{
    block::MemBlock,
    compression::dcl::{CompressionMode, DictType, compress_dcl, decompress_dcl},
};

fuzz_target!(|data: &[u8]| {
    let mut output = Vec::new();
    compress_dcl(
        CompressionMode::Binary,
        DictType::Size1024,
        data,
        &mut output,
    );

    let decompressed = decompress_dcl(&MemBlock::from_vec(output)).unwrap();
    assert_eq!(data, &*decompressed);
});
