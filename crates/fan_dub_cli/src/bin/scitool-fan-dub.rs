fn main() -> anyhow::Result<()> {
    let exec = smol::LocalExecutor::new();
    smol::block_on(exec.run(async {
        let system_path = scitool_fan_dub_cli::path::LookupPath::from_env();
        eprintln!("System PATH: {:?}", system_path.find_binary("ffmpeg"));
        let ffmpeg_tool = scitool_fan_dub_cli::tools::ffmpeg::FfmpegTool::from_path(
            system_path
                .find_binary("ffmpeg")
                .expect("ffmpeg not found in PATH")
                .to_path_buf(),
        );
        let file = smol::fs::File::open("/tmp/sample.wav").await?;
        ffmpeg_tool
            .convert(
                scitool_fan_dub_cli::tools::ffmpeg::ReaderInput::new(file),
                &std::path::PathBuf::from("/tmp/sample-2.mp3"),
                &mut scitool_fan_dub_cli::tools::ffmpeg::NullProgressListener,
            )
            .await?;
        Ok(())
    }))
}
