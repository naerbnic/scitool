use std::process::Stdio;

use tokio::select;

use crate::tools::{
    Tool,
    util::{CancelToken, ProcessAsyncReader},
};

pub struct EspeakTool {
    tool: Tool,
}

impl EspeakTool {
    #[must_use]
    pub fn from_tool(tool: Tool) -> Self {
        EspeakTool { tool }
    }

    pub fn synthesize(&self, text: &str) -> anyhow::Result<impl tokio::io::AsyncRead + 'static> {
        let mut child = self
            .tool
            .cmd_async()
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
