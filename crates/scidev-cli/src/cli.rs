mod messages;
mod project;
mod resources;
mod scripts;

use clap::Parser;

#[derive(Parser)]
pub(crate) struct Cli {
    #[clap(subcommand)]
    command: Cmd,
}

impl Cli {
    pub(crate) async fn run(&self) -> anyhow::Result<()> {
        match &self.command {
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
