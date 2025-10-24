use std::{collections::BTreeMap, path::PathBuf};

use clap::{Parser, Subcommand, builder::TypedValueParser};
use scidev::{
    resources::{ExtraData, ResourceId, ResourceSet, ResourceType, types::view::ViewHeader},
    utils::mem_reader::{BufferMemReader, MemReader},
};
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

    View(ProcessViewResources),
}

impl ResourceCommand {
    fn run(&self) -> anyhow::Result<()> {
        match self {
            ResourceCommand::List(list) => list.run()?,
            ResourceCommand::Extract(extract) => extract.run()?,
            ResourceCommand::Dump(dump) => dump.run()?,
            ResourceCommand::View(view) => view.run()?,
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
struct ProcessViewResources {
    /// Path to the game's root directory.
    root_dir: PathBuf,
}

impl ProcessViewResources {
    fn run(&self) -> anyhow::Result<()> {
        let resource_set = ResourceSet::from_root_dir(&self.root_dir)?;
        for resource in resource_set.resources_of_type(ResourceType::View) {
            if let Some(extra_data) = resource.extra_data() {
                let ExtraData::Composite {
                    ext_header,
                    extra_data,
                } = extra_data
                else {
                    panic!(
                        "Expected composite extra data for view resource {:?}",
                        resource.id()
                    );
                };

                let ext_data = ext_header.open_mem(..)?;
                let extra_data = extra_data.open_mem(..)?;
                let file_data = resource.load_data()?;

                eprintln!(
                    "Resource {:?} has extended header ({} bytes) and extra data ({} bytes)",
                    resource.id(),
                    ext_data.len(),
                    extra_data.len()
                );

                let mut reader = BufferMemReader::new(&*ext_data);
                assert!(extra_data.is_empty());

                let mut index = 0;
                while reader.remaining() > 0 {
                    let n16 = reader.read_u16_le()?;
                    println!("Index {index:4}: {n16:5?} (0x{n16:04x})");
                    index += 1;
                }

                let mut res_reader = BufferMemReader::new(&*file_data);
                let view_header = ViewHeader::read_from(&mut res_reader)?;
                eprintln!("View Header: {view_header:#x?}");

                // for loop_index in 0..view_header.loop_count {
                //     let mut loop_entry_data = res_reader.read_to_subreader(
                //         format!("View Loop {loop_index}"),
                //         view_header.loop_size.into(),
                //     )?;
                //     let loop_entry = LoopEntry::read_from(&mut loop_entry_data)?;
                //     eprintln!("View Loop {loop_index}: {loop_entry:#x?}");

                //     let cel_list_reader = BufferMemReader::new(&*file_data);
                //     let mut cel_list_reader = cel_list_reader.sub_reader_range(
                //         format!("Loop {loop_index} Cell Data"),
                //         usize::try_from(loop_entry.cel_offset).unwrap()..,
                //     )?;

                //     for cel_index in 0..loop_entry.cel_count {
                //         let mut cel_reader = cel_list_reader.read_to_subreader(
                //             format!("Cel {cel_index} Data"),
                //             view_header.cel_size.into(),
                //         )?;
                //         let cel_entry = CelEntry::read_from(&mut cel_reader)?;
                //         eprintln!("  Cel {cel_index}: {cel_entry:#x?}");
                //     }
                // }
            } else {
                // println!("Resource {:?} has no extra data", resource.id());
            }
        }
        Ok(())
    }
}
