use std::{collections::BTreeMap, path::PathBuf};

use clap::{Parser, Subcommand, builder::TypedValueParser};
use scidev::resources::{ResourceId, ResourceType};
use scitool_cli::commands::resources::{
    dump_resource, export, export_all, extract_resource_as_patch, list_resources,
};

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

    /// Exports a resource as a .respack file.
    Export(Export),

    /// Exports all resources as .respack files.
    ExportAll(ExportAll),
}

impl ResourceCommand {
    fn run(&self) -> anyhow::Result<()> {
        match self {
            ResourceCommand::List(list) => list.run()?,
            ResourceCommand::Extract(extract) => extract.run()?,
            ResourceCommand::Dump(dump) => dump.run()?,
            ResourceCommand::Export(export) => export.run()?,
            ResourceCommand::ExportAll(export_all) => export_all.run()?,
        }
        Ok(())
    }
}

static RESOURCE_NAME_MAP: std::sync::LazyLock<BTreeMap<&str, ResourceType>> =
    std::sync::LazyLock::new(|| {
        [
            ("view", ResourceType::View),
            ("pic", ResourceType::Pic),
            ("script", ResourceType::Script),
            ("scr", ResourceType::Script),
            ("text", ResourceType::Text),
            ("txt", ResourceType::Text),
            ("sound", ResourceType::Sound),
            ("memory", ResourceType::Memory),
            ("vocab", ResourceType::Vocab),
            ("voc", ResourceType::Vocab),
            ("font", ResourceType::Font),
            ("cursor", ResourceType::Cursor),
            ("patch", ResourceType::Patch),
            ("bitmap", ResourceType::Bitmap),
            ("palette", ResourceType::Palette),
            ("cdaudio", ResourceType::CdAudio),
            ("audio", ResourceType::Audio),
            ("sync", ResourceType::Sync),
            ("message", ResourceType::Message),
            ("msg", ResourceType::Message),
            ("map", ResourceType::Map),
            ("heap", ResourceType::Heap),
            ("audio36", ResourceType::Audio36),
            ("sync36", ResourceType::Sync36),
            ("translation", ResourceType::Translation),
            ("rave", ResourceType::Rave),
        ]
        .into()
    });

fn parse_resource_type() -> clap::builder::ValueParser {
    let possible_values =
        clap::builder::PossibleValuesParser::new(RESOURCE_NAME_MAP.keys().copied());
    possible_values.map(|s| RESOURCE_NAME_MAP[&*s]).into()
}

/// Lists resources in the game.
#[derive(Parser)]
struct ListResources {
    /// Path to the game's root directory.
    #[clap(index = 1)]
    root_dir: PathBuf,

    /// Filter by resource type (e.g., Script, Heap, View, Pic, Sound, Message, Font, Cursor, Patch, AudioPath, Vocab, Palette, Wave, Audio, Sync).
    #[clap(long = "type", short = 't', value_parser = parse_resource_type())]
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
    #[clap(long = "type", short = 't', value_parser = parse_resource_type())]
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
    #[clap(long = "type", short = 't', value_parser = parse_resource_type())]
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

#[derive(Parser)]
struct Export {
    root_dir: PathBuf,
    #[clap(long = "type", short = 't', value_parser = parse_resource_type())]
    resource_type: ResourceType,
    resource_id: u16,
    output_dir: PathBuf,
}

impl Export {
    fn run(&self) -> anyhow::Result<()> {
        let resource_id = ResourceId::new(self.resource_type, self.resource_id);
        export(&self.root_dir, resource_id, &self.output_dir)?;
        Ok(())
    }
}

#[derive(Parser)]
struct ExportAll {
    root_dir: PathBuf,
    output_root: PathBuf,
}

impl ExportAll {
    fn run(&self) -> anyhow::Result<()> {
        export_all(&self.root_dir, &self.output_root)?;
        Ok(())
    }
}
