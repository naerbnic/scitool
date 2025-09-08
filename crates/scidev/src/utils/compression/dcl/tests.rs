use crate::utils::block::MemBlock;

use super::*;

use proptest::prelude::*;

proptest! {
    #[test]
    fn compress_decompress_roundtrip(data in prop::collection::vec(any::<u8>(), 0..1_000_000)) {
        let mut compressed = Vec::new();
        compress_dcl(CompressionMode::Binary, DictType::Size1024, &data, &mut compressed);

        let decompressed = decompress_dcl(&MemBlock::from_vec(compressed)).unwrap();
        prop_assert_eq!(&*data, &*decompressed);
    }
}
