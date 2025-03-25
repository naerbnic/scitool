use scitool_fan_dub_cli::{path::LookupPath, tools::ffmpeg};

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
