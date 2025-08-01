mod csv;

use std::path::Path;

use scidev_book::Book;

fn load_book(book_path: &Path) -> anyhow::Result<Book> {
    let book: Book = scidev_book::file_format::deserialize_book(
        &mut serde_json::Deserializer::from_reader(std::fs::File::open(book_path)?),
    )?;
    Ok(book)
}

pub fn generate_csv(book_path: &Path, script_url: &str) -> anyhow::Result<String> {
    let book = load_book(book_path)?;
    Ok(self::csv::generate_csv(&book, script_url)?)
}
