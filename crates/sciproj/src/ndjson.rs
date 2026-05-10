use std::io;

use serde::Deserialize;

fn read_ndjson<'de, T: Deserialize<'de>, R: io::BufRead>(reader: R) -> anyhow::Result<Vec<T>> {
    todo!()

    // reader.read_until(buf)

    // for line in reader.lines() {
    //     let line = line?;
    //     serde_json::from_str(line.as_str())?;
    // }
    // for line in std::fs::read_to_string(path)?.lines() {
    //     result.push(serde_json::from_str(line)?);
    // }
    // Ok(result)
}
