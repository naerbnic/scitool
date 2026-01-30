mod book;
mod compile_audio;
mod export_scannable;
mod generate_csv;
mod messages;
mod project;
mod resources;
mod scripts;
mod try_scan;

use clap::Parser;

#[derive(Parser)]
pub(crate) struct Cli {
    #[clap(subcommand)]
    command: Cmd,
}

impl Cli {
    pub(crate) async fn run(&self) -> anyhow::Result<()> {
        match &self.command {
            Cmd::CompileAudio(compile_audio) => compile_audio.run()?,
            Cmd::ExportScannable(export_scannable) => export_scannable.run().await?,
            Cmd::TryScan(try_scan) => try_scan.run()?,
            Cmd::GenerateCsv(generate_csv) => generate_csv.run()?,
            Cmd::Book(book) => book.run()?,
            Cmd::Project(project) => project.run()?,
            Cmd::Resource(res) => res.run()?,
            Cmd::Message(msg) => msg.run()?,
            Cmd::Script(script) => script.run()?,
        }
        Ok(())
    }
}

#[derive(clap::Subcommand)]
enum Cmd {
    #[clap(name = "compile-audio")]
    CompileAudio(compile_audio::CompileAudio),
    #[clap(name = "export-scannable")]
    ExportScannable(export_scannable::ExportScannable),
    #[clap(name = "try-scan")]
    TryScan(try_scan::TryScan),
    #[clap(name = "generate-csv")]
    GenerateCsv(generate_csv::GenerateCsv),
    #[clap(name = "book")]
    Book(book::BookCommand),
    #[clap(name = "project", alias = "proj", alias = "p")]
    Project(project::Cmd),

    /// Commands for working with game resources.
    #[clap(name = "resources", visible_alias = "res")]
    Resource(resources::Resource),

    /// Commands for working with game messages.
    #[clap(name = "messages", visible_alias = "msg")]
    Message(messages::Messages),

    /// Commands for working with game scripts.
    #[clap(name = "script")]
    Script(scripts::Script),
}
