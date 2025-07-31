use clap::{Parser, Subcommand};

pub(crate) mod resources;

/// A command line tool for working with Sierra adventure games written in the SCI engine.
#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
pub(crate) struct Cli {
    /// The category of command to run.
    #[clap(subcommand)]
    category: Category,
}

impl Cli {
    pub(crate) fn run(&self) -> anyhow::Result<()> {
        self.category.run()
    }
}

/// The category of command to run.
#[derive(Subcommand)]
enum Category {
    #[clap(name = "res", about = "Commands for working with game resources.")]
    Resource(resources::Resource),
    #[clap(name = "msg", about = "Commands for working with game messages.")]
    Message(scitool_cli::cli::msg::Messages),
    #[clap(
        name = "gen",
        about = "Commands for generating various outputs from game data."
    )]
    Generate(scitool_cli::cli::generate::Generate),
    #[clap(name = "script", about = "Commands for working with game scripts.")]
    Script(scitool_cli::cli::script::Script),
    #[clap(name = "book", about = "Commands for working with game books.")]
    Book(scitool_cli::cli::book::BookCommand),
}

impl Category {
    fn run(&self) -> anyhow::Result<()> {
        match self {
            Category::Resource(res) => res.run(),
            Category::Message(msg) => msg.run(),
            Category::Generate(generate) => generate.run(),
            Category::Script(script) => script.run(),
            Category::Book(book) => book.run(),
        }
    }
}
