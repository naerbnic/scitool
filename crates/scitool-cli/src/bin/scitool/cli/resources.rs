use std::path::PathBuf;

use clap::{Parser, Subcommand};
use scidev_resources::{ResourceId, ResourceType};
use scitool_cli::commands::resources::{dump_resource, extract_resource_as_patch, list_resources};

/// Commands for working with game resources.
#[derive(Parser)]
pub(crate) struct Resource {
    /// The specific resource command to execute.
    #[clap(subcommand)]
    res_cmd: ResourceCommand,
}

impl Resource {
    pub(crate) fn run(&self) -> anyhow::Result<()> {
        self.res_cmd.run()
    }
}

/// The specific resource command to execute.
#[derive(Subcommand)]
enum ResourceCommand {
    /// Lists resources in the game.
    List(ListResources),

    /// Extracts a resource and saves it as a patch file. Supported types are Script (SCR) and Heap (HEP).
    Extract(ExtractResourceAsPatch),

    /// Dumps the hexadecimal content of a resource.
    Dump(DumpResource),
}

impl ResourceCommand {
    fn run(&self) -> anyhow::Result<()> {
        match self {
            ResourceCommand::List(list) => list.run()?,
            ResourceCommand::Extract(extract) => extract.run()?,
            ResourceCommand::Dump(dump) => dump.run()?,
        }
        Ok(())
    }
}

/// Lists resources in the game.
#[derive(Parser)]
struct ListResources {
    /// Path to the game's root directory.
    #[clap(index = 1)]
    root_dir: PathBuf,

    /// Filter by resource type (e.g., Script, Heap, View, Pic, Sound, Message, Font, Cursor, Patch, AudioPath, Vocab, Palette, Wave, Audio, Sync).
    #[clap(long = "type", short = 't')]
    res_type: Option<ResourceType>,
}

impl ListResources {
    fn run(&self) -> anyhow::Result<()> {
        let ids = list_resources(&self.root_dir, self.res_type)?;
        for id in ids {
            println!("{id:?}");
        }
        Ok(())
    }
}

/// Extracts a resource and saves it as a patch file. Supported types are Script (SCR) and Heap (HEP).
#[derive(Parser)]
#[allow(clippy::doc_markdown, reason = "Docstrings are converted to user help")]
struct ExtractResourceAsPatch {
    /// Path to the game's root directory.
    root_dir: PathBuf,

    /// The type of the resource to extract (e.g., Script, Heap).
    resource_type: ResourceType,

    /// The ID of the resource to extract.
    resource_id: u16,

    /// If set, prints what would be done without actually writing files.
    #[clap(short = 'n', long, default_value = "false")]
    dry_run: bool,

    /// Directory to save the output file. Defaults to <root_dir>.
    #[clap(short = 'o', long)]
    output_dir: Option<PathBuf>,
}

impl ExtractResourceAsPatch {
    fn run(&self) -> anyhow::Result<()> {
        let write_op = extract_resource_as_patch(
            &self.root_dir,
            self.resource_type,
            self.resource_id,
            self.output_dir.as_ref().unwrap_or(&self.root_dir),
        )?;
        if self.dry_run {
            eprintln!(
                "DRY_RUN: Writing resource {restype:?}:{resid} to {filename}",
                restype = write_op.resource_id.type_id(),
                resid = write_op.resource_id.resource_num(),
                filename = write_op.filename,
            );
        } else {
            eprintln!(
                "Writing resource {restype:?}:{resid} to {filename}",
                restype = write_op.resource_id.type_id(),
                resid = write_op.resource_id.resource_num(),
                filename = write_op.filename,
            );
            (write_op.operation)()?;
        }

        Ok(())
    }
}

/// Dumps the hexadecimal content of a resource.
#[derive(Parser)]
struct DumpResource {
    /// Path to the game's root directory.
    root_dir: PathBuf,

    /// The type of the resource to dump.
    resource_type: ResourceType,

    /// The ID of the resource to dump.
    resource_id: u16,
}

impl DumpResource {
    fn run(&self) -> anyhow::Result<()> {
        let resource_id = ResourceId::new(self.resource_type, self.resource_id);
        dump_resource(&self.root_dir, resource_id, std::io::stdout().lock())?;
        Ok(())
    }
}
