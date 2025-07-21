use clap::Parser;

mod cmds;

#[derive(Parser)]
struct Cli {
    #[clap(subcommand)]
    command: Cmd,
}

#[derive(clap::Subcommand)]
enum Cmd {
    #[clap(name = "compile-audio")]
    CompileAudio(cmds::CompileAudio),
    #[clap(name = "export-scannable")]
    ExportScannable(cmds::ExportScannable),
    #[clap(name = "try-scan")]
    TryScan(cmds::TryScan),
    Test,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Cli::parse();
    match &args.command {
        Cmd::CompileAudio(compile_audio) => compile_audio.run().await?,
        Cmd::ExportScannable(export_scannable) => export_scannable.run().await?,
        Cmd::TryScan(try_scan) => try_scan.run().await?,
        Cmd::Test => {
            scitool_fan_dub_cli::tools::gdrive::basic_flow().await?;
        }
    }
    Ok(())
}
