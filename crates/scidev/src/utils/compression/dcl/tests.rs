use std::io;

use crate::utils::compression::{
    dcl::decompress::DecompressDclProcessor, pipe::DataProcessor as _,
};

use super::*;

use proptest::prelude::*;

fn process_sync<R, W>(mut reader: R, mut writer: W) -> Result<(), DecompressionError>
where
    R: io::Read,
    W: io::Write,
{
    let mut processor = DecompressDclProcessor.push(&mut writer, 1024);
    io::copy(&mut reader, &mut processor)?;
    processor.close()?;
    Ok(())
}

fn process_pull<R, W>(mut reader: R, mut writer: W) -> Result<(), DecompressionError>
where
    R: io::Read,
    W: io::Write,
{
    let mut reader = DecompressDclProcessor.pull(&mut reader, 1024);
    io::copy(&mut reader, &mut writer)?;
    reader.close()?;
    Ok(())
}

fn process_push<R, W>(mut reader: R, mut writer: W) -> Result<(), DecompressionError>
where
    R: io::Read,
    W: io::Write,
{
    let mut writer = DecompressDclProcessor.push(&mut writer, 1024);
    io::copy(&mut reader, &mut writer)?;
    writer.close()?;
    Ok(())
}

fn do_process<'a, F>(f: F, buf: &'a [u8], out: &'a mut Vec<u8>) -> Result<(), DecompressionError>
where
    F: FnOnce(io::Cursor<&'a [u8]>, io::Cursor<&'a mut Vec<u8>>) -> Result<(), DecompressionError>,
{
    f(io::Cursor::new(buf), io::Cursor::new(out))?;
    Ok(())
}

proptest! {
    #[test]
    fn compress_decompress_roundtrip(data in prop::collection::vec(prop::sample::select(&[0u8, 1u8]), 0..10_000)) {
        let mut compressed = Vec::new();
        compress_dcl(CompressionMode::Binary, DictType::Size1024, &data, &mut compressed)?;

        let mut decompressed_sync = Vec::new();
        do_process(process_sync, &compressed, &mut decompressed_sync).unwrap();
        let mut decompressed_pull = Vec::new();
        do_process(process_pull, &compressed, &mut decompressed_pull).unwrap();
        let mut decompressed_push = Vec::new();
        do_process(process_push, &compressed, &mut decompressed_push).unwrap();
        prop_assert_eq!(&*data, &*decompressed_sync);
        prop_assert_eq!(&*data, &*decompressed_pull);
        prop_assert_eq!(&*data, &*decompressed_push);
    }
}

#[test]
fn compress_shrinks_data() -> io::Result<()> {
    let data = vec![0u8; 1_000_000];
    let mut compressed = Vec::new();
    compress_dcl(
        CompressionMode::Binary,
        DictType::Size1024,
        &data,
        &mut compressed,
    )?;
    assert!(
        compressed.len() < data.len(),
        "Compressed data should be smaller than original data: original size {}, compressed size {}",
        data.len(),
        compressed.len()
    );
    Ok(())
}

#[test]
fn empty_reader_roundtrip_works() -> io::Result<()> {
    let data: &[u8] = &[];
    let mut decompressed = Vec::new();
    let mut reader = decompress_reader(compress_reader(
        CompressionMode::Binary,
        DictType::Size1024,
        data,
    ));

    io::copy(&mut reader, &mut decompressed)?;

    assert_eq!(data, &*decompressed);
    Ok(())
}
