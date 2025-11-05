use clap::Parser;

mod book;
mod compile_audio;
mod export_scannable;
mod generate_csv;
mod project;
mod try_scan;

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
}
