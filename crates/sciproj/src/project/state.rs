mod schema;

use std::{
    fs::OpenOptions,
    io::{self, Write as _},
    path::Path,
};

pub(crate) struct StateFile {
    contents: schema::Contents,
}

impl StateFile {
    #[expect(dead_code, reason = "in progress")]
    pub(crate) fn open_at(path: &impl AsRef<Path>) -> std::io::Result<Self> {
        let state_path = path.as_ref().to_path_buf();
        let contents: schema::Contents = match std::fs::read(&state_path) {
            Ok(buf) => serde_json::from_slice(&buf).map_err(io::Error::other)?,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                let new_state_file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&state_path)?;
                let contents = schema::Contents::default();
                let buf = serde_json::to_vec_pretty(&contents).map_err(io::Error::other)?;
                (&new_state_file).write_all(&buf)?;
                drop(new_state_file);
                contents
            }
            Err(e) => return Err(e),
        };
        Ok(StateFile { contents })
    }

    #[expect(dead_code)]
    pub(crate) fn save_to(&self, path: &impl AsRef<Path>) -> std::io::Result<()> {
        let buf = serde_json::to_vec_pretty(&self.contents).map_err(io::Error::other)?;
        let tempfile = tempfile::NamedTempFile::new_in(
            path.as_ref()
                .parent()
                .ok_or_else(|| io::Error::other("invalid state path"))?,
        )?;
        (&tempfile).write_all(&buf)?;
        tempfile.persist(path)?;
        Ok(())
    }
}
