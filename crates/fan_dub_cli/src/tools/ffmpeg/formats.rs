//! File formats we support

use std::{borrow::Cow, collections::HashMap, ffi::OsString};

pub struct FlacOutputOptions {
    compression_level: u8,
}

impl FlacOutputOptions {
    pub fn get_options(&self) -> AVOptions {
        let mut options = HashMap::new();
        options.insert(
            "compression_level".into(),
            self.compression_level.to_string(),
        );
        AVOptions(options)
    }
}

pub struct Mp3OutputOptions {
    bitrate: u32,
}

impl Mp3OutputOptions {
    pub fn get_options(&self) -> AVOptions {
        let mut options = HashMap::new();
        options.insert("bitrate".into(), self.bitrate.to_string());
        AVOptions(options)
    }
}

pub struct OggVorbisOutputOptions {
    quality: u32,
    sample_rate: Option<u32>,
}

impl OggVorbisOutputOptions {
    pub fn new(quality: u32, sample_rate: impl Into<Option<u32>>) -> Self {
        OggVorbisOutputOptions {
            quality,
            sample_rate: sample_rate.into(),
        }
    }

    #[must_use]
    pub fn get_options(&self) -> AVOptions {
        let mut options = HashMap::new();
        options.insert("q".into(), self.quality.to_string());
        if let Some(sample_rate) = self.sample_rate {
            options.insert("ar".into(), sample_rate.to_string());
        }
        AVOptions(options)
    }
}

impl Default for OggVorbisOutputOptions {
    fn default() -> Self {
        OggVorbisOutputOptions::new(4, None)
    }
}

pub enum OutputFormat {
    Flac(FlacOutputOptions),
    Mp3(Mp3OutputOptions),
    Ogg(OggVorbisOutputOptions),
}

impl OutputFormat {
    #[must_use]
    pub fn format_name(&self) -> &'static str {
        match self {
            OutputFormat::Flac(_) => "flac",
            OutputFormat::Mp3(_) => "mp3",
            OutputFormat::Ogg(_) => "ogg",
        }
    }
    #[must_use]
    pub fn get_options(&self) -> AVOptions {
        match self {
            OutputFormat::Flac(opts) => opts.get_options(),
            OutputFormat::Mp3(opts) => opts.get_options(),
            OutputFormat::Ogg(opts) => opts.get_options(),
        }
    }
}

impl From<FlacOutputOptions> for OutputFormat {
    fn from(opts: FlacOutputOptions) -> Self {
        OutputFormat::Flac(opts)
    }
}

impl From<Mp3OutputOptions> for OutputFormat {
    fn from(opts: Mp3OutputOptions) -> Self {
        OutputFormat::Mp3(opts)
    }
}

impl From<OggVorbisOutputOptions> for OutputFormat {
    fn from(opts: OggVorbisOutputOptions) -> Self {
        OutputFormat::Ogg(opts)
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
