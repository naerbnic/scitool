use std::io;

mod schema;

/// The root config file for sciproj.
pub(crate) struct ConfigFile {
    #[expect(dead_code)]
    contents: schema::Contents,
}
impl ConfigFile {
    pub(crate) fn open_at(path: &impl AsRef<std::path::Path>) -> io::Result<Self> {
        let data = std::fs::read_to_string(path)?;
        let contents =
            toml::from_str(&data).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        Ok(ConfigFile { contents })
    }
}
