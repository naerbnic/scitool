use std::{
    fmt::Debug,
    pin::Pin,
    process::{Output, Stdio},
};

use tokio::{io::AsyncRead, process::Command, select};

use crate::{
    path::abspath::AbsPathBuf,
    tools::{
        TestableTool,
        location::ToolLocation,
        util::{CancelToken, ProcessAsyncReader},
    },
};

pub type BoxEspeak = Box<dyn Espeak + Send + Sync>;

pub trait Espeak: TestableTool + Debug {
    fn synthesize(&self, text: &str) -> anyhow::Result<Pin<Box<dyn AsyncRead + Send>>>;
}

pub fn from_tool_location(location: &ToolLocation) -> anyhow::Result<BoxEspeak> {
    let bin_path = location.bin_path();
    let data_path = match location {
        ToolLocation::System(_) => None,
        ToolLocation::Dist(loc) => Some(loc.install_root().join("share/espeak-ng-data")),
    };
    Ok(Box::new(EspeakTool::from_abs_path(bin_path, data_path)))
}

#[derive(Debug)]
pub struct EspeakTool {
    bin_path: AbsPathBuf,
    data_path: Option<AbsPathBuf>,
}

impl EspeakTool {
    #[must_use]
    fn from_abs_path(bin_path: AbsPathBuf, data_path: Option<AbsPathBuf>) -> Self {
        EspeakTool {
            bin_path,
            data_path,
        }
    }

    pub fn synthesize(
        &self,
        text: &str,
    ) -> anyhow::Result<impl tokio::io::AsyncRead + Send + 'static> {
        let mut cmd = Command::new(&self.bin_path);
        cmd.arg("--stdout")
            .arg("--")
            .arg(text)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        if let Some(data_path) = &self.data_path {
            cmd.env("ESPEAK_DATA_PATH", data_path);
        }

        let mut child = cmd.spawn()?;

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
    async fn test_binary_impl(&self) -> anyhow::Result<Output> {
        let mut cmd = Command::new(&self.bin_path);
        Ok(cmd.arg("--version").output().await?)
    }
}

impl TestableTool for EspeakTool {
    fn test_binary<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<Output>> + Send + 'a>> {
        let result = self.test_binary_impl();
        Box::pin(result)
    }
}

impl Espeak for EspeakTool {
    fn synthesize(&self, text: &str) -> anyhow::Result<Pin<Box<dyn AsyncRead + Send>>> {
        let result = self.synthesize(text)?;
        Ok(Box::pin(result))
    }
}
