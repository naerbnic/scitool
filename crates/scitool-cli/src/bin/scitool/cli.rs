#![allow(clippy::doc_markdown, reason = "Docstrings are converted to user help")]
use clap::{Parser, Subcommand};

mod messages;
mod resources;
mod scripts;

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

impl Category {
    fn run(&self) -> anyhow::Result<()> {
        match self {
            Category::Resource(res) => res.run(),
            Category::Message(msg) => msg.run(),
            Category::Script(script) => script.run(),
        }
    }
}
