use std::{path::PathBuf, process::Stdio};

use tokio::select;

use crate::tools::util::{CancelToken, ProcessAsyncReader};

pub struct EspeakTool {
    espeak_path: PathBuf,
}

impl EspeakTool {
    #[must_use]
    pub fn from_path(espeak_path: PathBuf) -> Self {
        EspeakTool { espeak_path }
    }

    pub fn synthesize(&self, text: &str) -> anyhow::Result<impl tokio::io::AsyncRead + 'static> {
        let mut child = tokio::process::Command::new(&self.espeak_path)
            .arg("--stdout")
            .arg("--")
            .arg(text)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;

        let stdout = child.stdout.take().expect("No stdout");

        let (token, fut) = CancelToken::new(async move |h| {
            let exit_value = select! {
                exit_value = child.wait() => {
                    exit_value?
                }
                () = h.children_dropped() => {
                    child.kill().await?;
                    child.wait().await?
                }
            };
            if !exit_value.success() {
                return Err(anyhow::anyhow!("Failed to synthesize audio: {exit_value}"));
            }
            Ok::<_, anyhow::Error>(())
        });

        tokio::spawn(fut);

        Ok(ProcessAsyncReader::new(stdout, token))
    }
}
