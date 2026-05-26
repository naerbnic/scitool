use std::{collections::BTreeMap, fmt::Display, path::PathBuf, str::FromStr, time::Duration};

use clap::{Parser, Subcommand};
use scidev::{ids::LineId, utils::serde::Sha256Hash};
use sciproj::{
    book::{RoleId, config::BookConfig, file_format},
    build::audio::compile_audio_base,
    path::relpath::{RelPath, RelPathBuf, Segment},
    resources::{AudioClip, load_book_from_resources},
};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error};

use crate::{
    commands::config::ProjectConfig,
    data::{ConfigFormat, DataFormat, load_config, load_data, store_data},
};

#[derive(Debug, Parser)]
pub(crate) struct Cli {
    #[command(subcommand)]
    command: Command,
}

impl Cli {
    pub(crate) fn run(self) -> anyhow::Result<()> {
        self.command.run()
    }
}

#[derive(Debug, Subcommand)]
enum Command {
    Script(ScriptSubCommand),
    Build(Build),
    GameData(GameData),
}

impl Command {
    fn run(self) -> anyhow::Result<()> {
        match self {
            Self::Script(s) => s.run(),
            Self::Build(build) => build.run(),
            Self::GameData(game_data) => game_data.run(),
        }
    }
}

#[derive(Debug, Parser)]
struct ScriptSubCommand {
    #[command(subcommand)]
    sub_command: ScriptCommand,
}

impl ScriptSubCommand {
    fn run(self) -> anyhow::Result<()> {
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

struct ProjectEnv {
    project_root: PathBuf,
    config: ProjectConfig,
    game_path: PathBuf,
    script_config: BookConfig,
}

#[derive(Debug, clap::Args)]
struct GlobalConfigArgs {
    #[arg(long)]
    project_root: Option<PathBuf>,
}

impl GlobalConfigArgs {
    fn load_env(self) -> anyhow::Result<ProjectEnv> {
        let project_root = if let Some(project_root) = self.project_root {
            project_root
        } else {
            crate::commands::config::find_project_root(&std::env::current_dir()?)?
        };

        let config = crate::commands::config::load_project(&project_root)?;

        // Load the book from game resources.
        let game_path = config.game_files().to_std_path(&project_root);
        let voice_script_config_path = config.voice_script_config().to_std_path(&project_root);

        let script_config: BookConfig =
            load_config(&voice_script_config_path, &ConfigFormat::Toml)?;
        Ok(ProjectEnv {
            project_root,
            config,
            game_path,
            script_config,
        })
    }
}

struct ToFromStringSerde;

impl ToFromStringSerde {
    pub(crate) fn serialize<T, S>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        T: ToString,
        S: Serializer,
    {
        value.to_string().serialize(serializer)
    }

    pub(crate) fn deserialize<'de, T, D>(deserializer: D) -> Result<T, D::Error>
    where
        T: FromStr,
        T::Err: Display,
        D: Deserializer<'de>,
    {
        let s: &str = Deserialize::deserialize(deserializer)?;
        T::from_str(s).map_err(D::Error::custom)
    }
}

#[derive(Debug, Deserialize)]
struct LineMapping {
    #[serde(with = "ToFromStringSerde")]
    line_id: LineId,
    clip_path: RelPathBuf,
    clip_start_ns: Option<u64>,
    clip_end_ns: Option<u64>,
}

#[derive(Debug, Parser)]
pub(crate) struct Build {
    #[command(flatten)]
    env: GlobalConfigArgs,
    /// The name of the game target to export.
    #[arg(index = 1, conflicts_with = "line_mapping", required = true)]
    target: Option<String>,

    #[arg(short = 'm', long, required = true)]
    line_mapping: Option<PathBuf>,

    #[arg(long)]
    audio_files: Option<PathBuf>,

    #[arg(short, long)]
    output: Option<PathBuf>,

    #[arg(short = 'n', long)]
    dry_run: bool,
}

impl Build {
    pub(crate) fn run(self) -> anyhow::Result<()> {
        let env = self.env.load_env()?;

        let book = load_book_from_resources(&env.script_config, &env.game_path)?;

        let (line_mapping_path, target_name) = if let Some(target) = &self.target {
            let Some(target_config) = env.config.targets().get(target) else {
                anyhow::bail!("Target {target} not found")
            };
            (
                target_config.line_mapping().to_std_path(&env.project_root),
                target.as_str(),
            )
        } else {
            let Some(line_mapping) = &self.line_mapping else {
                unreachable!("ensured by clap");
            };
            (line_mapping.clone(), "default")
        };

        let line_mappings: Vec<LineMapping> =
            load_data(&line_mapping_path, &crate::data::DataFormat::Csv)?;

        let audio_files_root = self.audio_files.unwrap_or_else(|| {
            env.config
                .audio_files_root()
                .unwrap_or(RelPath::new(""))
                .to_std_path(&env.project_root)
        });

        let mut line_map = BTreeMap::new();
        for line_mapping in line_mappings {
            let clip = AudioClip {
                start_us: line_mapping.clip_start_ns,
                end_us: line_mapping.clip_end_ns,
                path: line_mapping.clip_path.to_std_path(&audio_files_root),
            };

            line_map
                .insert(line_mapping.line_id, clip)
                .ok_or_else(|| anyhow::anyhow!("Duplicate line from project."))?;
        }

        // Check that every line in the book is present in the line map.
        let mut non_present_clips = Vec::new();

        for line in book.lines() {
            let id = line.id();
            if !line_map.contains_key(&id) {
                non_present_clips.push(id);
            }
        }

        let mut unused_clips = Vec::new();
        for id in line_map.keys() {
            if book.get_line(*id).is_none() {
                unused_clips.push(*id);
            }
        }

        // TODO: Have an option to generate a placeholder
        if !non_present_clips.is_empty() {
            anyhow::bail!(
                "The following {} lines are not present in the line map: {}",
                non_present_clips.len(),
                non_present_clips
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<String>>()
                    .join(", ")
            );
        }

        if !unused_clips.is_empty() {
            anyhow::bail!(
                "The following {} clips are not used in the book: {}",
                unused_clips.len(),
                unused_clips
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<String>>()
                    .join(", ")
            );
        }

        let target_relpath = Segment::try_new(target_name)
            .ok_or_else(|| anyhow::anyhow!("Target name must be a valid relative path segment"))?;

        let base_output_dir = self.output.unwrap_or_else(|| {
            env.config
                .build_dir()
                .unwrap_or(RelPath::new("build"))
                .join(target_relpath)
                .to_std_path(&env.project_root)
        });

        if self.dry_run {
            return Ok(());
        }

        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        rt.block_on(compile_audio_base(&line_map, &base_output_dir))?;

        rt.shutdown_timeout(Duration::from_secs(1));

        Ok(())
    }
}

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
        }
    }
}

#[derive(Debug, Subcommand)]
enum ExportScriptSubcommand {
    Lines(ExportLines),
    Book(ExportBook),
}

#[derive(Debug, Serialize, Deserialize)]
struct ScriptLineRecord {
    #[serde(with = "ToFromStringSerde")]
    id: LineId,
    line_text: String,
    #[serde(with = "ToFromStringSerde")]
    role: RoleId,
}

#[derive(Debug, Parser)]
struct ExportLines {
    #[command(flatten)]
    env: GlobalConfigArgs,

    #[clap(short = 'o', long, required = true)]
    output: PathBuf,
}

impl ExportLines {
    fn run(self) -> anyhow::Result<()> {
        let env = self.env.load_env()?;
        let book = load_book_from_resources(&env.script_config, &env.game_path)?;
        let mut script_lines = Vec::new();
        for line in book.lines() {
            script_lines.push(ScriptLineRecord {
                id: line.id(),
                line_text: line.text().to_plain_text(),
                role: line.role().id(),
            });
        }

        store_data(&self.output, &script_lines[..], &DataFormat::Csv)?;
        Ok(())
    }
}

#[derive(Debug, Parser)]
struct ExportBook {
    #[command(flatten)]
    env: GlobalConfigArgs,

    #[clap(short = 'o', long, required = true)]
    output: PathBuf,
}

impl ExportBook {
    fn run(self) -> anyhow::Result<()> {
        let env = self.env.load_env()?;
        let book = load_book_from_resources(&env.script_config, &env.game_path)?;

        let output = std::fs::File::create(&self.output)?;

        file_format::serialize_book(&book, &mut serde_json::Serializer::new(output))?;
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(transparent)]
struct Manifest(BTreeMap<PathBuf, Sha256Hash>);

const MANIFEST_FILE_NAME: &str = "scidub.manifest.json";

#[derive(Debug, Parser)]
struct GameData {
    #[clap(subcommand)]
    command: GameDataSubcommand,
}

impl GameData {
    fn run(self) -> anyhow::Result<()> {
        match self.command {
            GameDataSubcommand::CreateManifest(c) => c.run(),
        }
    }
}

#[derive(Debug, Subcommand)]
enum GameDataSubcommand {
    CreateManifest(CreateManifest),
}

#[derive(Debug, Parser)]
struct CreateManifest {
    #[command(flatten)]
    env: GlobalConfigArgs,
}

impl CreateManifest {
    fn run(self) -> anyhow::Result<()> {
        let env = self.env.load_env()?;

        let walker = walkdir::WalkDir::new(&env.game_path)
            .follow_links(false)
            .follow_root_links(false);

        let mut manifest = Manifest(BTreeMap::new());
        for entry in walker {
            let entry = entry?;
            if entry.file_type().is_file() {
                let file = std::fs::File::open(entry.path())?;
                let (hash, _) = Sha256Hash::from_stream_hash(file)?;
                let relative_path = entry.path().strip_prefix(&env.game_path)?;
                manifest.0.insert(relative_path.to_path_buf(), hash);
            }
        }

        let manifest_path = env.project_root.join(MANIFEST_FILE_NAME);

        let mut output = std::fs::File::create(&manifest_path)?;
        serde_json::to_writer_pretty(&mut output, &manifest)?;
        Ok(())
    }
}
