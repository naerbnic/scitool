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
}

async fn async_main() -> anyhow::Result<()> {
    let args = Cli::parse();
    match &args.command {
        Cmd::CompileAudio(compile_audio) => compile_audio.run().await?,
        Cmd::ExportScannable(export_scannable) => export_scannable.run().await?,
    }
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let exec = smol::LocalExecutor::new();
    smol::block_on(exec.run(async_main()))
}
