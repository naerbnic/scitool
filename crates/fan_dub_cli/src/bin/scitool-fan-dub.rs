use std::path::PathBuf;

use sci_resources::types::msg::MessageId;
use scitool_fan_dub_cli::{path::LookupPath, tools::ffmpeg};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct AudioClip {
    pub start_us: u64,
    pub end_us: u64,
    pub path: PathBuf,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Sample {
    pub room: u16,
    pub message_id: MessageId,
    pub clip: AudioClip,
}

#[derive(clap::Parser)]
struct Cli {
    #[clap(subcommand)]
    command: Cmd,
}

#[derive(clap::Subcommand)]
enum Cmd {
    #[clap(name = "compile-audio")]
    CompileAudio(CompileAudio),
}

#[derive(clap::Parser)]
struct CompileAudio {
    #[clap(short = 'd', long)]
    sample_dir: PathBuf,
}

impl CompileAudio {
    pub fn run(&self) -> anyhow::Result<()> {
        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    let exec = smol::LocalExecutor::new();
    smol::block_on(exec.run(async {
        let system_path = LookupPath::from_env();
        eprintln!("System PATH: {:?}", system_path.find_binary("ffmpeg"));
        let ffmpeg_tool = ffmpeg::FfmpegTool::from_path(
            system_path
                .find_binary("ffmpeg")
                .expect("ffmpeg not found in PATH")
                .to_path_buf(),
        );
        let file = smol::fs::File::open("/tmp/sample-2.mp3").await?;
        let data = ffmpeg_tool
            .convert(
                ffmpeg::ReaderInput::new(file),
                ffmpeg::VecOutput,
                ffmpeg::OggVorbisOutputOptions::new(128 * 1000),
                &mut ffmpeg::NullProgressListener,
            )
            .await?;
        eprintln!("Converted data size: {}", data.len());
        smol::fs::write("/tmp/sample-3.mp3", &data).await?;
        Ok(())
    }))
}
