use clap::Parser;
use serde::Serialize;
use std::{
    io,
    path::{Path, PathBuf},
};

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

fn load_book(book_path: &Path) -> anyhow::Result<Book> {
    let book: Book = scidev_book::file_format::deserialize_book(
        &mut serde_json::Deserializer::from_reader(std::fs::File::open(book_path)?),
    )?;
    Ok(book)
}

fn generate_csv(book_path: &Path, script_url: &str) -> anyhow::Result<String> {
    let book = load_book(book_path)?;
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

#[derive(Parser)]
pub(super) struct GenerateCsv {
    book_path: PathBuf,

    /// Base URL for the game script page.
    #[clap(long, default_value = "https://sq5-fan-dub.github.io/script")]
    base_url: String,
}

impl GenerateCsv {
    pub(super) fn run(&self) -> anyhow::Result<()> {
        let csv = generate_csv(&self.book_path, &self.base_url)?;
        println!("{csv}");
        Ok(())
    }
}
