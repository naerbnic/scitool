use std::path::PathBuf;

use clap::{Parser, Subcommand};

use scitool_cli::commands::messages::{LineFilter, check_messages, for_each_line, print_talkers};

/// Prints messages from the game, with optional filters.
#[derive(Parser)]
struct PrintMessages {
    /// Path to the book file.
    #[clap(index = 1)]
    book_path: PathBuf,
    /// Filter by talker ID.
    #[clap(short = 't', long, required = false)]
    talker: Option<u8>,
    /// Filter by room ID.
    #[clap(short = 'r', long, required = false)]
    room: Option<u16>,
    /// Filter by verb ID.
    #[clap(short = 'v', long, required = false)]
    verb: Option<u8>,
    /// Filter by noun ID.
    #[clap(short = 'n', long, required = false)]
    noun: Option<u8>,
    /// Filter by condition ID.
    #[clap(short = 'c', long, required = false)]
    condition: Option<u8>,
    /// Filter by sequence ID.
    #[clap(short = 's', long, required = false)]
    sequence: Option<u8>,
}

impl PrintMessages {
    fn run(&self) -> anyhow::Result<()> {
        let filter = LineFilter::new(
            self.talker,
            self.room,
            self.verb,
            self.noun,
            self.condition,
            self.sequence,
        );
        for_each_line(&self.book_path, &filter, |line| {
            println!(
                "(room: {:?}, n: {:?}, v: {:?}, c: {:?}, s: {:?}, t: {:?}):",
                line.id().room_num(),
                line.id().noun_num(),
                line.id().verb_num(),
                line.id().condition_num(),
                line.id().sequence_num(),
                line.talker_num(),
            );
            let text = line.text().to_plain_text().replace("\r\n", "\n    ");
            println!("    {}", text.trim());
            Ok(())
        })?;
        Ok(())
    }
}

/// Checks message data, building a "book" and printing statistics and validation errors.
#[derive(Parser)]
struct CheckMessages {
    /// Path to a book file.
    book_path: PathBuf,
}

impl CheckMessages {
    fn run(&self) -> anyhow::Result<()> {
        check_messages(&self.book_path, std::io::stderr().lock())
    }
}

/// Prints a list of all unique talker IDs found in the game messages.
#[derive(Parser)]
struct PrintTalkers {
    /// Path to the game's root directory.
    #[clap(index = 1)]
    root_dir: PathBuf,
}

impl PrintTalkers {
    fn run(&self) -> anyhow::Result<()> {
        print_talkers(&self.root_dir, std::io::stdout().lock())
    }
}

/// The specific message command to execute.
#[derive(Subcommand)]
enum MessageCommand {
    #[clap(about = "Prints messages from the game, with optional filters.")]
    Print(PrintMessages),
    #[clap(
        about = "Checks message data, building a \"book\" and printing statistics and validation errors."
    )]
    Check(CheckMessages),
    #[clap(
        name = "print-talkers",
        about = "Prints a list of all unique talker IDs found in the game messages."
    )]
    PrintTalkers(PrintTalkers),
}

/// Commands for working with game messages.
#[derive(Parser)]
pub(super) struct Messages {
    /// The specific message command to execute.
    #[clap(subcommand)]
    msg_cmd: MessageCommand,
}

impl Messages {
    pub(super) fn run(&self) -> anyhow::Result<()> {
        match &self.msg_cmd {
            MessageCommand::Print(cmd) => cmd.run()?,
            MessageCommand::Check(cmd) => cmd.run()?,
            MessageCommand::PrintTalkers(cmd) => cmd.run()?,
        }
        Ok(())
    }
}
