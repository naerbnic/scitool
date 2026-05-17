use std::io::Cursor;

use itertools::Itertools;
use scidev_errors::{define_error, ensure, prelude::*};

define_error! {
    pub struct NdJsonError;
}

pub(crate) fn parse_ndjson<'de, T>(data: &'de [u8]) -> Result<Vec<T>, NdJsonError>
where
    T: serde::Deserialize<'de>,
{
    let mut next_was_empty = false;
    let mut result = Vec::new();
    for (curr, next) in data.split(|b| *b == b'\n').tuple_windows() {
        ensure!(
            !curr.is_empty(),
            "Had blank line in the middle of ndjson data."
        );
        result.push(
            serde_json::from_slice(curr)
                .raise()
                .msg("Deserialization error")?,
        );
        next_was_empty = next.is_empty();
    }

    ensure!(
        next_was_empty,
        "ndjson data must be terminated with a newline."
    );

    Ok(result)
}

pub(crate) fn serialize_ndjson<T>(items: &[T]) -> Result<Vec<u8>, NdJsonError>
where
    T: serde::Serialize,
{
    let mut result = Vec::new();
    for item in items {
        let mut item_ser = Vec::new();
        item.serialize(&mut serde_json::ser::Serializer::new(&mut Cursor::new(
            &mut item_ser,
        )))
        .raise()
        .msg("while deserializing item")?;

        ensure!(
            !item_ser.iter().contains(&b'\n'),
            "serialized item contained a newline in its representation."
        );

        result.extend_from_slice(&item_ser);
        result.push(b'\n');
    }
    Ok(result)
}
