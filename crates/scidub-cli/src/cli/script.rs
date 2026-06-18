use std::{io::Write as _, path::PathBuf};

use clap::{Parser, Subcommand};
use scidev::ids::{ConversationId, LineId};
use sciproj::book::{RoleId, file_format};
use serde::{Deserialize, Serialize};

use crate::{
    cli::GlobalConfigArgs,
    data::{DataFormat, ToFromStringSerde, store_data},
};

/// Work with game voice scripts.
#[derive(Debug, Parser)]
pub(super) struct ScriptSubCommand {
    #[command(subcommand)]
    sub_command: ScriptCommand,
}

impl ScriptSubCommand {
    pub(super) fn run(self) -> anyhow::Result<()> {
        self.sub_command.run()
    }
}

#[derive(Debug, Subcommand)]
enum ScriptCommand {
    Export(ExportScript),
}

impl ScriptCommand {
    fn run(self) -> anyhow::Result<()> {
        match self {
            Self::Export(export) => export.run(),
        }
    }
}

/// Export the game script or related data.
#[derive(Debug, Parser)]
struct ExportScript {
    #[clap(subcommand)]
    command: ExportScriptSubcommand,
}

impl ExportScript {
    fn run(self) -> anyhow::Result<()> {
        match self.command {
            ExportScriptSubcommand::Lines(export_lines) => export_lines.run(),
            ExportScriptSubcommand::Book(export_book) => export_book.run(),
            ExportScriptSubcommand::Schema(export_schema) => export_schema.run(),
        }
    }
}

#[derive(Debug, Subcommand)]
enum ExportScriptSubcommand {
    Lines(ExportLines),
    Book(ExportBook),
    Schema(ExportSchema),
}

#[derive(Debug, Serialize, Deserialize)]
struct ScriptLineUrlRecords {
    #[serde(with = "ToFromStringSerde")]
    id: LineId,
    #[serde(with = "ToFromStringSerde")]
    conv_id: ConversationId,
    #[serde(with = "ToFromStringSerde")]
    role_id: RoleId,
    line_url: String,
    conv_url: String,
    line_text: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ScriptLineRecord {
    #[serde(with = "ToFromStringSerde")]
    id: LineId,
    #[serde(with = "ToFromStringSerde")]
    conv_id: ConversationId,
    #[serde(with = "ToFromStringSerde")]
    role_id: RoleId,
    line_text: String,
}

/// Exports all spoken lines in a tabular format.
///
/// Includes a unique ID for each line, the role ID for the line, and the
/// text of the line.
///
/// The default format is CSV, which includes a header row. Field names are
/// `id`, `role_id`, and `line_text`
#[derive(Debug, Parser)]
struct ExportLines {
    #[command(flatten)]
    env: GlobalConfigArgs,

    #[clap(short = 'o', long, required = true)]
    output: PathBuf,
}

impl ExportLines {
    fn run(self) -> anyhow::Result<()> {
        let project = self.env.load_project()?;

        let config = project.config()?;
        let book = project.book()?;
        if let Some(script_url) = config.script_url() {
            let mut script_lines = Vec::new();
            for line in book.lines() {
                let conv = line.conversation();
                script_lines.push(ScriptLineUrlRecords {
                    id: line.id(),
                    conv_id: conv.id(),
                    line_url: format!("{}#{}", script_url, line.id()),
                    conv_url: format!("{}#{}", script_url, conv.id()),
                    role_id: line.role().id(),
                    line_text: line.text().to_plain_text(),
                });
            }

            store_data(&self.output, &script_lines[..], &DataFormat::Csv)?;
        } else {
            let mut script_lines = Vec::new();
            for line in book.lines() {
                let conv = line.conversation();
                script_lines.push(ScriptLineRecord {
                    id: line.id(),
                    conv_id: conv.id(),
                    role_id: line.role().id(),
                    line_text: line.text().to_plain_text(),
                });
            }

            store_data(&self.output, &script_lines[..], &DataFormat::Csv)?;
        }
        Ok(())
    }
}

/// Exports a Book file in JSON format.
///
/// This generates a representation of the entire voice script that can be
/// used in the VO Script web app. Lines are extracted from the game data.
#[derive(Debug, Parser)]
struct ExportBook {
    #[command(flatten)]
    env: GlobalConfigArgs,

    /// The file to write the book to.
    #[clap(short = 'o', long, required = true)]
    output: PathBuf,
}

impl ExportBook {
    fn run(self) -> anyhow::Result<()> {
        let project = self.env.load_project()?;
        let book = project.book()?;

        let output = std::fs::File::create(&self.output)?;

        file_format::serialize_book(book, &mut serde_json::Serializer::new(output))?;
        Ok(())
    }
}

/// Write the JSON Schema of the book format to stdout or a given file.
///
/// This is used by other packages to generate code to read the format.
#[derive(Debug, Parser)]
struct ExportSchema {
    /// If set, pretty-prints the schema output.
    #[clap(short, long, default_value = "false")]
    pretty: bool,

    /// If set, the schema is written to the given file.
    #[clap(short, long)]
    output: Option<PathBuf>,
}

impl ExportSchema {
    fn run(self) -> anyhow::Result<()> {
        let json_schema = file_format::json_schema(self.pretty);
        let writer: Box<dyn std::io::Write> = if let Some(out_path) = self.output.as_ref() {
            Box::new(std::fs::File::create(out_path)?)
        } else {
            Box::new(std::io::stdout())
        };
        let mut writer = std::io::BufWriter::new(writer);
        writer.write_all(json_schema.as_bytes())?;
        writer.flush()?;
        Ok(())
    }
}
