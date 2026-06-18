use clap::{Parser, ValueEnum};
use tracing::Level;

use crate::dist_env::DistEnv;

#[derive(ValueEnum, Clone, Debug)]
enum RequiredTool {
    #[clap(name = "espeak")]
    EpeakNG,
    #[clap(name = "ffmpeg")]
    Ffmpeg,
    #[clap(name = "scinc")]
    Scinc,
}

#[derive(Debug)]
enum ToolStatus {
    Unavailable,
    Invalid,
    Valid,
}

/// Checks that the distribution environment is properly set up.
/// Should generally not be necessary outside of building for distribution.
#[derive(Debug, Parser)]
pub(super) struct CheckDistribution {
    #[clap(long)]
    test_fail: bool,

    #[clap(long, value_delimiter = ',')]
    required_tools: Vec<RequiredTool>,
}

impl CheckDistribution {
    #[tracing::instrument]
    async fn run_inner(self) -> anyhow::Result<()> {
        let mut is_valid = true;
        let env = DistEnv::try_load_env()?;
        let ffmpeg_tool = env.ffmpeg_tool()?;
        let espeak_tool = env.espeak_tool()?;
        let scinc_tool = env.scinc_tool()?;
        let ffmpeg_status = if let Some(ffmpeg) = ffmpeg_tool.as_deref() {
            eprintln!("ffmpeg binary found. Execution params: {ffmpeg:?}");
            let output = ffmpeg.test_binary().await?;
            if output.status.success() {
                eprintln!(
                    "ffmpeg found. Version info:\n{}",
                    String::from_utf8_lossy(&output.stdout)
                );
            } else {
                eprintln!(
                    "Unable to run ffmpeg: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            if output.status.success() {
                ToolStatus::Valid
            } else {
                ToolStatus::Invalid
            }
        } else {
            ToolStatus::Unavailable
        };
        let espeak_status = if let Some(espeak) = espeak_tool.as_deref() {
            eprintln!("espeak binary found. Execution params: {espeak:?}");
            let output = espeak.test_binary().await?;
            if output.status.success() {
                eprintln!(
                    "espeak-ng found. Version info:\n{}",
                    String::from_utf8_lossy(&output.stdout)
                );
            } else {
                eprintln!(
                    "Unable to run espeak-ng: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            if output.status.success() {
                ToolStatus::Valid
            } else {
                ToolStatus::Invalid
            }
        } else {
            ToolStatus::Unavailable
        };
        let scinc_status = if let Some(scinc) = scinc_tool.as_deref() {
            eprintln!("scinc binary found. Execution params: {scinc:?}");
            let output = scinc.test_binary().await?;
            if output.status.success() {
                eprintln!(
                    "scinc found. Version info:\n{}",
                    String::from_utf8_lossy(&output.stdout)
                );
            } else {
                eprintln!(
                    "Unable to run scinc: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            if output.status.success() {
                ToolStatus::Valid
            } else {
                ToolStatus::Invalid
            }
        } else {
            ToolStatus::Unavailable
        };
        for req_tool in &self.required_tools {
            match req_tool {
                RequiredTool::Ffmpeg => {
                    is_valid &= matches!(ffmpeg_status, ToolStatus::Valid);
                }
                RequiredTool::EpeakNG => {
                    is_valid &= matches!(espeak_status, ToolStatus::Valid);
                }
                RequiredTool::Scinc => {
                    is_valid &= matches!(scinc_status, ToolStatus::Valid);
                }
            }
        }

        if self.test_fail {
            is_valid = false;
            eprintln!("Testing check fail");
        }

        eprintln!("DEPENDENCY STATUSES:");
        eprintln!("  FFmpeg: {ffmpeg_status:?}");
        eprintln!("  espeak-ng: {espeak_status:?}");
        eprintln!("  scinc: {scinc_status:?}");

        anyhow::ensure!(is_valid, "Distribution environment is not properly set up.");
        Ok(())
    }
    pub(super) fn run(self) -> anyhow::Result<()> {
        // We explicitly want to see any tracing steps during environment setup.
        tracing::subscriber::set_global_default(
            tracing_subscriber::fmt()
                .with_writer(std::io::stderr)
                .with_max_level(Level::INFO)
                .finish(),
        )?;
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        rt.block_on(self.run_inner())
    }
}
