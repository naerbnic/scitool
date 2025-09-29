use std::{path::PathBuf, str::FromStr};

use futures::TryFutureExt as _;
use serde::Deserialize;

use super::input::InputState;

fn de_from_string<'de, T, D>(deserializer: D) -> Result<T, D::Error>
where
    T: FromStr,
    T::Err: std::fmt::Display,
    D: serde::de::Deserializer<'de>,
{
    use serde::de::Error;
    let s: &str = <&str>::deserialize(deserializer)?;
    s.parse::<T>().map_err(Error::custom)
}

fn de_opt_from_string<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: FromStr,
    T::Err: std::fmt::Display,
    D: serde::de::Deserializer<'de>,
{
    use serde::de::Error;
    let s: Option<&str> = Option::<&str>::deserialize(deserializer)?;
    match s {
        Some(s) => s.parse::<T>().map(Some).map_err(Error::custom),
        None => Ok(None),
    }
}

// A serde-parsable representation of the output of ffprobe. Intended to be
// use with JSON output
#[derive(Debug, Clone, Deserialize)]
struct ProbeOutput {
    format: Option<FormatData>,
}

#[derive(Debug, Clone, Deserialize)]
struct FormatData {
    #[serde(deserialize_with = "de_from_string")]
    duration: f64,
    #[serde(deserialize_with = "de_opt_from_string")]
    #[expect(dead_code)]
    size: Option<u64>,
    #[serde(deserialize_with = "de_opt_from_string")]
    #[expect(dead_code)]
    bit_rate: Option<u64>,
}

pub(crate) struct Probe {
    path: PathBuf,
}

impl Probe {
    pub(crate) fn new(path: PathBuf) -> Self {
        Probe { path }
    }

    pub(crate) fn read_duration(&self, input: impl super::Input) -> anyhow::Result<f64> {
        let rt = tokio::runtime::Builder::new_current_thread().build()?;
        let in_state = rt.block_on(input.create_state())?;
        let mut command = tokio::process::Command::new(&self.path);
        command
            .arg("-i")
            .arg(in_state.url())
            .args(["-v", "error"])
            .args(["-show_entries", "format"])
            .args(["-of", "json"]);
        let (output, ()) = rt.block_on(async move {
            futures::try_join!(
                command.output().map_err(anyhow::Error::from),
                in_state.wait()
            )
        })?;
        anyhow::ensure!(
            output.status.success(),
            "ffprobe failed with code {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr),
        );
        let out: ProbeOutput = serde_json::from_slice(&output.stdout)?;
        out.format
            .map(|f| f.duration)
            .ok_or_else(|| anyhow::anyhow!("ffprobe did not return format data: {}", output.status))
    }
}
