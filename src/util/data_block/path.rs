use std::{fs::File, path::PathBuf};

use super::{io_source::IoSource, BlockSource, ReadBlock, WriteBlock};

#[derive(Debug, Clone)]
pub struct PathBlockSource {
    file_path: PathBuf,
}

impl PathBlockSource {
    pub fn new(file_path: PathBuf) -> Self {
        PathBlockSource { file_path }
    }
}

impl BlockSource for PathBlockSource {
    fn open_read(&self) -> super::Result<Box<dyn ReadBlock>> {
        let file = File::open(&self.file_path)?;
        Ok(IoSource::new(file).into_read_box())
    }

    fn open_write(&self) -> super::Result<Box<dyn WriteBlock>> {
        Ok(IoSource::new(
            std::fs::OpenOptions::new()
                .append(false)
                .write(true)
                .create(false)
                .open(&self.file_path)?,
        )
        .into_write_box())
    }
}
