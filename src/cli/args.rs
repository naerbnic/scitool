//! Helper functions for parsing command line arguments.

use std::{ffi::OsString, fs::File};

#[derive(Clone)]
pub struct OutFilePath(OutFileType);

impl OutFilePath {
    pub fn new_stdout() -> Self {
        OutFilePath(OutFileType::Stdout)
    }

    pub fn new_path(path: OsString) -> Self {
        OutFilePath(OutFileType::File(path))
    }

    pub fn open(&self) -> std::io::Result<Box<dyn std::io::Write>> {
        match &self.0 {
            OutFileType::File(path) => Ok(Box::new(std::io::BufWriter::new(File::create(path)?))),
            OutFileType::Stdout => Ok(Box::new(std::io::stdout().lock())),
        }
    }
}

impl From<OsString> for OutFilePath {
    fn from(s: OsString) -> Self {
        if s == "-" {
            Self::new_stdout()
        } else {
            Self::new_path(s)
        }
    }
}

#[derive(Clone)]
enum OutFileType {
    /// The output is a file.
    File(OsString),
    /// The output is stdout.
    Stdout,
}
