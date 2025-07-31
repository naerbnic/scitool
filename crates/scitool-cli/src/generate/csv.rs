use serde::Serialize;
use std::io;

use scidev_book::Book;

#[derive(Serialize)]
struct LineRecord {
    #[serde(rename = "Line Id")]
    line_id: String,
    #[serde(rename = "Conversation Id")]
    conv_id: String,
    #[serde(rename = "Line URL")]
    line_url: String,
    #[serde(rename = "Conversation URL")]
    conv_url: String,
    #[serde(rename = "Role")]
    role: String,
    #[serde(rename = "Text")]
    text: String,
}

pub(crate) fn generate_csv(book: &Book, script_url: &str) -> io::Result<String> {
    let output = io::Cursor::new(Vec::<u8>::new());
    let mut csv_writer = csv::Writer::from_writer(output);
    for line in book.lines() {
        let line_record = LineRecord {
            line_id: line.id().to_string(),
            conv_id: line.id().conv_id().to_string(),
            line_url: format!("{}#{}", script_url, line.id()),
            conv_url: format!("{}#{}", script_url, line.id().conv_id()),
            role: line.role().id().to_string(),
            text: line.text().to_plain_text(),
        };
        csv_writer.serialize(line_record)?;
    }
    csv_writer.flush()?;
    Ok(String::from_utf8(csv_writer.into_inner().unwrap().into_inner()).unwrap())
}
