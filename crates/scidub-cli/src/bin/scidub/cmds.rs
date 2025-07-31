use clap::Parser;

mod compile_audio;
mod export_scannable;
mod try_scan;

#[derive(Parser)]
struct Cli {
    #[clap(subcommand)]
    command: Cmd,
}

#[derive(clap::Subcommand)]
enum Cmd {
    #[clap(name = "compile-audio")]
    CompileAudio(compile_audio::CompileAudio),
    #[clap(name = "export-scannable")]
    ExportScannable(export_scannable::ExportScannable),
    #[clap(name = "try-scan")]
    TryScan(try_scan::TryScan),
}

pub(crate) async fn async_main() -> anyhow::Result<()> {
    let args = Cli::parse();
    match &args.command {
        Cmd::CompileAudio(compile_audio) => compile_audio.run().await?,
        Cmd::ExportScannable(export_scannable) => export_scannable.run().await?,
        Cmd::TryScan(try_scan) => try_scan.run().await?,
    }
    Ok(())
}
