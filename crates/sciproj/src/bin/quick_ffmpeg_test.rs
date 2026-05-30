use sciproj::tools::ffmpeg::OggVorbisOutputOptions;
use tokio::io::AsyncWriteExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let wav_input = tokio::fs::File::open("/tmp/test.wav").await?;

    let mut output = tokio::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open("/tmp/test.ogg")
        .await?;
    let start_time = tokio::time::Instant::now();
    let mut converted = sciproj::tools::ffmpeg::ConverterReader::new(
        wav_input,
        "/opt/homebrew/bin/ffmpeg",
        sciproj::tools::ffmpeg::OutputFormat::Ogg(OggVorbisOutputOptions::new(4, None)),
        None,
        None,
    )
    .await?;

    tokio::io::copy(&mut converted, &mut output).await?;

    output.shutdown().await?;
    let duration = tokio::time::Instant::now() - start_time;
    eprintln!("Conversion took {duration:?}");

    Ok(())
}
