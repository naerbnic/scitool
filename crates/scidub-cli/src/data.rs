//! Functions for handling paths to data files, with multiple formats.

use std::{
    io::{BufWriter, Read as _},
    path::Path,
};

use anyhow::Context;
use sciproj::formats::ndjson::{parse_ndjson, serialize_ndjson};

#[derive(Clone)]
pub(crate) enum DataFormat {
    Json,
    Csv,
    NdJson,
}

impl DataFormat {
    fn from_ext(ext: &str) -> Option<Self> {
        Some(match ext {
            "json" => DataFormat::Json,
            "csv" => DataFormat::Csv,
            "ndjson" => DataFormat::NdJson,
            _ => return None,
        })
    }
}

pub(crate) fn load_data<T>(
    path: impl AsRef<Path>,
    default_format: &DataFormat,
) -> anyhow::Result<Vec<T>>
where
    T: serde::de::DeserializeOwned,
{
    let path = path.as_ref();
    let format = path
        .extension()
        .and_then(|ext| ext.to_str().and_then(DataFormat::from_ext))
        .unwrap_or_else(|| default_format.clone());

    load_data_as(path, &format)
}

pub(crate) fn load_data_as<T>(path: impl AsRef<Path>, format: &DataFormat) -> anyhow::Result<Vec<T>>
where
    T: serde::de::DeserializeOwned,
{
    let path = path.as_ref();

    let mut file = std::fs::File::open(path)?;

    let data = match format {
        DataFormat::Json => serde_json::from_reader(file).context(format!(
            "Could not parse data file as JSON: {}",
            path.display()
        ))?,
        DataFormat::Csv => {
            let reader = csv::ReaderBuilder::new()
                .has_headers(true)
                .from_reader(file);
            reader.into_deserialize().collect::<Result<Vec<_>, _>>()?
        }
        DataFormat::NdJson => {
            let mut contents = Vec::new();
            file.read_to_end(&mut contents)
                .context(format!("Could not read data file: {}", path.display()))?;
            parse_ndjson(&contents)?
        }
    };

    Ok(data)
}

pub(crate) fn store_data<T>(
    path: impl AsRef<Path>,
    data: &[T],
    format: &DataFormat,
) -> anyhow::Result<()>
where
    T: serde::Serialize,
{
    let path = path.as_ref();
    let file = std::fs::File::create(path)?;

    match format {
        DataFormat::Json => serde_json::to_writer(file, data).context(format!(
            "Could not parse data file as JSON: {}",
            path.display()
        ))?,
        DataFormat::Csv => {
            let mut writer = csv::WriterBuilder::new()
                .has_headers(true)
                .from_writer(BufWriter::new(file));
            for item in data {
                writer.serialize(item)?;
            }
            writer.flush().context("Error flushing CSV writer")?;
        }
        DataFormat::NdJson => {
            let bytes = serialize_ndjson(data).context(format!(
                "Could not serialize data to NDJSON format: {}",
                path.display()
            ))?;
            std::fs::write(path, bytes)
                .context(format!("Could not write data file: {}", path.display()))?;
        }
    }

    Ok(())
}

#[derive(Debug, Clone)]
pub(crate) enum ConfigFormat {
    Json,
    Yaml,
    Toml,
}

impl ConfigFormat {
    fn from_ext(ext: &str) -> Option<Self> {
        Some(match ext {
            "json" => Self::Json,
            "yaml" | "yml" => Self::Yaml,
            "toml" => Self::Toml,
            _ => return None,
        })
    }
}

pub(crate) fn load_config<T>(
    path: impl AsRef<Path>,
    default_format: &ConfigFormat,
) -> anyhow::Result<T>
where
    T: serde::de::DeserializeOwned,
{
    let path = path.as_ref();
    let format = path
        .extension()
        .and_then(|ext| ext.to_str().and_then(ConfigFormat::from_ext))
        .unwrap_or_else(|| default_format.clone());
    load_config_as(path, &format)
}

pub(crate) fn load_config_as<T>(path: impl AsRef<Path>, format: &ConfigFormat) -> anyhow::Result<T>
where
    T: serde::de::DeserializeOwned,
{
    let path = path.as_ref();

    let mut file = std::fs::File::open(path)?;

    let data = match format {
        ConfigFormat::Json => serde_json::from_reader(file).context(format!(
            "Could not parse data file as JSON: {}",
            path.display()
        ))?,
        ConfigFormat::Toml => {
            let mut contents = Vec::new();
            file.read_to_end(&mut contents)
                .context(format!("Could not read data file: {}", path.display()))?;
            toml::from_slice(&contents).context(format!(
                "Could not parse data file as TOML: {}",
                path.display()
            ))?
        }
        ConfigFormat::Yaml => serde_norway::from_reader(file).context(format!(
            "Could not parse data file as YAML: {}",
            path.display()
        ))?,
    };

    Ok(data)
}
