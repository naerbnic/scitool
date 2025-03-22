//! File formats we support

use std::{borrow::Cow, collections::HashMap, ffi::OsString};

pub enum AudioFormat {
    Mp3,
    Flac,
    Wav,
}

impl AudioFormat {
    pub fn input_format_flags(&self) -> Vec<OsString> {
        match self {
            AudioFormat::Mp3 => vec!["-f".into(), "mp3".into()],
            AudioFormat::Flac => vec!["-f".into(), "flac".into()],
            AudioFormat::Wav => vec!["-f".into(), "wav".into()],
        }
    }

    pub fn output_format_flags(&self) -> Vec<OsString> {
        match self {
            AudioFormat::Mp3 => vec!["-f".into(), "mp3".into()],
            AudioFormat::Flac => vec!["-f".into(), "flac".into()],
            AudioFormat::Wav => vec![
                "-f".into(),
                "wav".into(),
                "-acodec".into(),
                "pcm_s16le".into(),
            ],
        }
    }
}

pub struct FlacOutputFormat {
    compression_level: u8,
}

impl FlacOutputFormat {
    pub fn get_options(&self) -> AVOptions {
        let mut options = HashMap::new();
        options.insert(
            "compression_level".into(),
            self.compression_level.to_string(),
        );
        AVOptions(options)
    }
}

pub struct Mp3OutputFormat {
    bitrate: u32,
}

impl Mp3OutputFormat {
    pub fn get_options(&self) -> AVOptions {
        let mut options = HashMap::new();
        options.insert("bitrate".into(), self.bitrate.to_string());
        AVOptions(options)
    }
}

pub enum OutputFormat {
    Flac(FlacOutputFormat),
    Mp3(Mp3OutputFormat),
}

impl OutputFormat {
    pub fn get_options(&self) -> AVOptions {
        match self {
            OutputFormat::Flac(format) => {
                let mut options = HashMap::new();
                options.insert(
                    "compression_level".into(),
                    format.compression_level.to_string(),
                );
                AVOptions(options)
            }
            OutputFormat::Mp3(format) => {
                let mut options = HashMap::new();
                options.insert("bitrate".into(), format.bitrate.to_string());
                AVOptions(options)
            }
        }
    }
}

pub struct AVOptions(HashMap<Cow<'static, str>, String>);

impl AVOptions {
    pub fn to_flags(&self, stream_spec: Option<&str>) -> Vec<OsString> {
        let mut flags = Vec::new();
        for (key, value) in &self.0 {
            let flag = if let Some(stream_spec) = stream_spec {
                format!("-{key}:{stream_spec}")
            } else {
                format!("-{key}")
            };
            flags.push(flag.into());
            flags.push(value.into());
        }
        flags
    }
}
