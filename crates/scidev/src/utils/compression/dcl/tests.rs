use crate::utils::block::MemBlock;

use super::*;

use proptest::prelude::*;

proptest! {
    #[test]
    fn compress_decompress_roundtrip(data in prop::collection::vec(prop::sample::select(&[0u8, 1u8]), 0..10_000)) {
        let mut compressed = Vec::new();
        compress_dcl(CompressionMode::Binary, DictType::Size1024, &data, &mut compressed);

        let decompressed = decompress_dcl(&MemBlock::from_vec(compressed)).unwrap();
        prop_assert_eq!(&*data, &*decompressed);
    }
}

#[test]
fn compress_shrinks_data() {
    let data = vec![0u8; 1_000_000];
    let mut compressed = Vec::new();
    compress_dcl(
        CompressionMode::Binary,
        DictType::Size1024,
        &data,
        &mut compressed,
    );
    assert!(
        compressed.len() < data.len(),
        "Compressed data should be smaller than original data: original size {}, compressed size {}",
        data.len(),
        compressed.len()
    );
}
