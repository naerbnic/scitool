use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Display,
    io::Write as _,
    path::{Path, PathBuf},
    str::FromStr,
    time::Duration,
};

use anyhow::Context;
use clap::{Parser, Subcommand, ValueEnum};
use csv::WriterBuilder;
use indicatif::ProgressFinish;
use scidev::ids::{ConversationId, LineId};
use sciproj::{
    book::{RoleId, file_format},
    build::audio::{ProgressFactory, compile_audio_base},
    path::relpath::{RelPath, RelPathBuf, Segment},
    resources::AudioClip,
    tools::{espeak::EspeakTool, ffmpeg::FfmpegTool},
};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error};

use crate::{
    data::{DataFormat, load_data, store_data},
    dist_env::DistEnv,
    project::{Manifest, Project},
};

/// A utility for managing and building an SCI fan-dub project.
#[derive(Debug, Parser)]
#[clap(version, about)]
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
    Init(Init),
    Script(ScriptSubCommand),
    Build(Build),
    GameData(GameData),
    #[clap(hide = true)]
    CheckDistribution(CheckDistribution),
}

impl Command {
    fn run(self) -> anyhow::Result<()> {
        match self {
            Self::Init(init) => init.run(),
            Self::Script(s) => s.run(),
            Self::Build(build) => build.run(),
            Self::GameData(game_data) => game_data.run(),
            Self::CheckDistribution(c) => c.run(),
        }
    }
}

struct RelWalkDir {
    base: PathBuf,
    iter: walkdir::IntoIter,
}

impl RelWalkDir {
    fn new(base: impl AsRef<Path>) -> Self {
        let base = base.as_ref().to_path_buf();
        Self {
            iter: walkdir::WalkDir::new(&base)
                .follow_links(false)
                .follow_root_links(false)
                .into_iter(),
            base,
        }
    }
}

impl Iterator for RelWalkDir {
    type Item = anyhow::Result<RelPathBuf>;

    fn next(&mut self) -> Option<Self::Item> {
        (|| {
            Ok(Some(loop {
                let Some(entry) = self.iter.next().transpose()? else {
                    return Ok(None);
                };

                if !entry.file_type().is_file() {
                    continue;
                }

                let path = entry.path().strip_prefix(&self.base)?;
                break RelPath::try_from_std_path(&path)
                    .ok_or_else(|| anyhow::anyhow!("Invalid relative path: {}", path.display()))?
                    .to_buf();
            }))
        })()
        .transpose()
    }
}

fn generate_manifest_from_game_dir(source: impl AsRef<Path>) -> anyhow::Result<Manifest> {
    let source = source.as_ref();
    let walker = RelWalkDir::new(source);
    let mut manifest = Manifest::new();
    for path in walker {
        let path = path?;
        let file = std::fs::File::open(source.join(&path))?;
        manifest.add_from_reader(path, file)?;
    }
    Ok(manifest)
}

#[derive(Debug, Default)]
struct ValidationResult {
    matches: BTreeSet<RelPathBuf>,
    mismatches: BTreeSet<RelPathBuf>,
    missing: BTreeSet<RelPathBuf>,
    additional: BTreeSet<RelPathBuf>,
}

impl ValidationResult {
    /// Validate that the directory contains all of the files in the manifest,
    /// unchanged from the version that is in the manifest itself.
    fn validate_complete(&self) -> anyhow::Result<()> {
        if !self.mismatches.is_empty() || !self.missing.is_empty() {
            anyhow::bail!(
                "Files in manifest are not complete. Missing files: {:?}, Mismatched Files: {:?}",
                &self.missing,
                &self.mismatches
            )
        }

        Ok(())
    }

    /// Returns an iterator over the files that are in the directory, but not
    /// mentioned in the manifest.
    fn additional_files(&self) -> impl Iterator<Item = &RelPath> {
        self.additional.iter().map(RelPathBuf::as_path)
    }
}

/// Given a manifest, checks how the files in the directory match against the
/// contents of the manifest.
fn validate_manifest_in_game_dir(
    source: impl AsRef<Path>,
    manifest: &Manifest,
) -> anyhow::Result<ValidationResult> {
    let source = source.as_ref();
    let mut remaining_files: BTreeSet<&RelPath> = manifest.entries().keys().map(|p| &**p).collect();

    let mut matches = BTreeSet::new();
    let mut mismatches = BTreeSet::new();
    let mut additional = BTreeSet::new();
    for path in RelWalkDir::new(source) {
        let path = path?;
        if !remaining_files.remove(path.as_path()) {
            additional.insert(path);
            continue;
        }

        let file_matches = manifest.match_file(&path, std::fs::File::open(source.join(&path))?)?;

        if file_matches {
            matches.insert(path);
        } else {
            mismatches.insert(path);
        }
    }

    let missing = remaining_files.into_iter().map(RelPath::to_buf).collect();

    Ok(ValidationResult {
        matches,
        mismatches,
        missing,
        additional,
    })
}

const INIT_GIT_IGNORE: &str = "\
# Generated by `scidub init`

# Ensure that the default `build/` directory is not tracked. This directory
# is used to store files for building the project.
/build/

# The `game_files` directory is where the original game files are stored.
# It is intentionally untracked to avoid committing potentially copyrighted
# content into the project.
/game_files/

# Ensure the directory is kept using the .gitkeep file.
!/game_files/.gitkeep
";

const INIT_SCIDUB_TOML: &str = "
# Generated by `scidub init`
game_data = \"game_data\"
";

/// Creates a new project.
///
/// This generates an initial project configuration, directory structure,
/// and .gitignore file.
///
/// If a path to the game data is provided, it will also generate an initial
/// manifest file for the game resources.
#[derive(Debug, Parser)]
struct Init {
    /// The root directory for the new project.
    ///
    /// If not specified, the current working directory is used.
    #[clap(index = 1)]
    root: Option<PathBuf>,

    /// A path to the game data files to be copied into the project.
    ///
    /// If provided, the files will be copied to the `game_data/` directory
    /// under the project root. By default, this directory will not be
    /// tracked by Git (or other tools that observe .gitignore) to avoid
    /// checking in potentially copyrighted files into a Git project.
    #[clap(long)]
    game_data: Option<PathBuf>,
}

impl Init {
    fn run(self) -> anyhow::Result<()> {
        let root = self.root.map_or_else(std::env::current_dir, Ok)?;

        anyhow::ensure!(root.exists(), "Project directory does not exist.");
        anyhow::ensure!(
            std::fs::read_dir(&root)?.next().is_none(),
            "Project directory is not empty."
        );

        eprintln!("Creating project under {}", root.display());
        let git_ignore_path = root.join(".gitignore");
        let project_config_path = root.join("scidub.toml");
        if !git_ignore_path.exists() {
            std::fs::write(&git_ignore_path, INIT_GIT_IGNORE)?;
        }

        if !project_config_path.exists() {
            std::fs::write(&git_ignore_path, INIT_SCIDUB_TOML)?;
        }

        let game_data_path = root.join("game_data");
        std::fs::create_dir_all(&game_data_path)?;
        std::fs::write(game_data_path.join(".gitkeep"), "")?;

        if let Some(source_data) = &self.game_data {
            for path in RelWalkDir::new(source_data) {
                let path = path?;
                let source_path = source_data.join(&path);
                let dest_path = game_data_path.join(&path);
                if let Some(dest_parent) = dest_path.parent() {
                    std::fs::create_dir_all(dest_parent)?;
                    std::fs::copy(&source_path, &dest_path)?;
                }
            }
        }

        let manifest = generate_manifest_from_game_dir(&game_data_path)?;

        // We are in a state where we can load the project directly.
        let project = Project::new(root);

        let mut output = std::fs::File::create(project.manifest_path()?)?;
        serde_json::to_writer_pretty(&mut output, &manifest)?;

        Ok(())
    }
}

/// Work with game voice scripts.
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

/// Common flag arguments for all project-based commands.
#[derive(Debug, clap::Args)]
struct GlobalConfigArgs {
    /// Provides an explicit root for the project.
    #[arg(long)]
    project_root: Option<PathBuf>,
}

impl GlobalConfigArgs {
    /// Load a project from the command line flags and/or current process
    /// environment.
    fn load_project(self) -> anyhow::Result<Project> {
        let project = if let Some(project_root) = self.project_root {
            Project::new(project_root)
        } else {
            Project::new_from_path(&std::env::current_dir()?)?
        };

        Ok(project)
    }
}

/// A serde-based wrapper for types that implement `ToString` and `FromStr`.
///
/// This can be provided for the `with` attribute on `Deserialize` and `Serialize`.
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

#[derive(Debug, Serialize)]
struct MissingLine {
    #[serde(with = "ToFromStringSerde")]
    missing_line_id: LineId,
}

#[expect(clippy::extra_unused_lifetimes)]
fn line_mappings_to_clip_map<'a>(
    line_mappings: impl IntoIterator<Item = LineMapping>,
    audio_files_root: &Path,
) -> anyhow::Result<BTreeMap<LineId, AudioClip>> {
    let mut line_map = BTreeMap::new();
    for line_mapping in line_mappings {
        let clip = AudioClip {
            start_us: line_mapping.clip_start_ns,
            end_us: line_mapping.clip_end_ns,
            path: line_mapping.clip_path.to_std_path(audio_files_root),
        };

        line_map
            .insert(line_mapping.line_id, clip)
            .ok_or_else(|| anyhow::anyhow!("Duplicate line from project."))?;
    }
    Ok(line_map)
}

/// Builds a resource patch from a target.
#[derive(Debug, Parser)]
pub(crate) struct Build {
    #[command(flatten)]
    env: GlobalConfigArgs,

    /// The name of the game target to export.
    #[arg(index = 1, conflicts_with = "line_mapping", required = true)]
    target: Option<String>,

    /// The path to the line mapping data file
    ///
    /// This can be JSON, YAML, or CSV with a header.
    #[arg(short = 'm', long, required = true)]
    line_mapping: Option<PathBuf>,

    /// The root path for the paths to audio files.
    ///
    /// If not provided, the directory from the configuration will be used. If not
    /// specified in the project, the project directory will be used.
    #[arg(long)]
    audio_files: Option<PathBuf>,

    /// The directory to write the generated patch files to.
    ///
    /// If not provided, the configured target path will be used. If not
    /// specified in the project, the "build/" directory under the project
    /// directory will be used.
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// If true, don't actually create any files.
    #[arg(short = 'n', long)]
    dry_run: bool,
}

impl Build {
    pub(crate) fn run(self) -> anyhow::Result<()> {
        let project = self.env.load_project()?;

        let config = project.config()?;
        let book = project.book()?;

        let (line_mapping_path, target_name) = if let Some(target) = &self.target {
            let Some(target_config) = config.targets().get(target) else {
                anyhow::bail!("Target {target} not found")
            };
            (
                target_config.line_mapping().to_std_path(project.root()),
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
            config
                .audio_files_root()
                .unwrap_or(RelPath::new(""))
                .to_std_path(project.root())
        });

        let clip_map = line_mappings_to_clip_map(line_mappings, &audio_files_root)?;

        let target_relpath = Segment::try_new(target_name)
            .ok_or_else(|| anyhow::anyhow!("Target name must be a valid relative path segment"))?;

        let base_output_dir = if let Some(output_dir) = self.output {
            output_dir
        } else {
            project.build_dir()?.join(target_relpath)
        };
        std::fs::create_dir_all(&base_output_dir)?;

        // Check that every line in the book is present in the line map.
        let mut non_present_clips = Vec::new();

        for line in book.lines() {
            let id = line.id();
            if !clip_map.contains_key(&id) {
                non_present_clips.push(id);
            }
        }

        let mut unused_clips = Vec::new();
        for id in clip_map.keys() {
            if book.get_line(*id).is_none() {
                unused_clips.push(*id);
            }
        }

        if !non_present_clips.is_empty() {
            let missing_lines_path = base_output_dir.join("missing_lines.csv");

            non_present_clips.sort_unstable();
            eprintln!("There were lines in the project that are not provided in the mapping.");
            eprintln!("Writing missing lines to {}", missing_lines_path.display());

            let mut missing_lines_writer = WriterBuilder::new()
                .has_headers(true)
                .from_path(missing_lines_path)?;
            for line_id in non_present_clips {
                missing_lines_writer.serialize(MissingLine {
                    missing_line_id: line_id,
                })?;
            }
            missing_lines_writer.flush()?;
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

        if self.dry_run {
            return Ok(());
        }

        let res_dir_path = base_output_dir.join("res");
        std::fs::create_dir_all(&res_dir_path)?;

        let stderr_term = console::Term::buffered_stderr();
        let progress_factory =
            ProgressFactory::new(stderr_term).with_finish(ProgressFinish::AndLeave);

        let dist_env = DistEnv::from_env();

        let ffmpeg_tool = FfmpegTool::from_tool(dist_env.ffmpeg_tool());
        let espeak_tool = dist_env.espeak_tool().map(EspeakTool::from_tool);
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        rt.block_on(compile_audio_base(
            &ffmpeg_tool,
            espeak_tool.as_ref(),
            progress_factory,
            book,
            &clip_map,
            &res_dir_path,
        ))?;

        rt.shutdown_timeout(Duration::from_secs(1));

        Ok(())
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

/// Manage game resource data.
#[derive(Debug, Parser)]
struct GameData {
    #[clap(subcommand)]
    command: GameDataSubcommand,
}

impl GameData {
    fn run(self) -> anyhow::Result<()> {
        match self.command {
            GameDataSubcommand::CreateManifest(c) => c.run(),
            GameDataSubcommand::Validate(v) => v.run(),
            GameDataSubcommand::Import(i) => i.run(),
        }
    }
}

#[derive(Debug, Subcommand)]
enum GameDataSubcommand {
    CreateManifest(CreateManifest),
    Validate(ValidateManifest),
    Import(ImportGame),
}

/// Create a manifest for the current game data.
///
/// To ensure that any resources are being taken from the same source,
/// without checking in any resources, the project can track a
/// manifest file, which records the set of files in the game data, as well
/// as its hash.
#[derive(Debug, Parser)]
struct CreateManifest {
    #[command(flatten)]
    env: GlobalConfigArgs,
}

impl CreateManifest {
    fn run(self) -> anyhow::Result<()> {
        let project = self.env.load_project()?;
        let game_path = project.game_path()?;
        let manifest_path = project.manifest_path()?;
        let manifest = generate_manifest_from_game_dir(game_path)?;

        let mut output = std::fs::File::create(manifest_path)?;
        serde_json::to_writer_pretty(&mut output, &manifest)?;
        Ok(())
    }
}

/// Validate that the current game data matches the manifest.
///
/// This command can be used to detect if the game data has changed since
/// the last time the manifest was created.
#[derive(Debug, Parser)]
struct ValidateManifest {
    #[command(flatten)]
    env: GlobalConfigArgs,
}

impl ValidateManifest {
    fn run(self) -> anyhow::Result<()> {
        let project = self
            .env
            .load_project()
            .context("while loading project".to_string())?;
        let manifest = project.manifest()?;
        let game_path = project.game_path()?;

        let result = validate_manifest_in_game_dir(game_path, manifest)?;

        result.validate_complete()?;

        if result.additional_files().next().is_some() {
            eprintln!(
                "Found additional files in game directory: {}",
                result
                    .additional_files()
                    .map(|p| p.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }

        eprintln!("Files at {} match the manifest.", game_path.display());
        Ok(())
    }
}

/// Imports the resources of a source game into the project directory.
///
/// If there is a manifest, will copy only the files in the manifest, verifying
/// that the files match before copying.
#[derive(Debug, Parser)]
struct ImportGame {
    #[command(flatten)]
    env: GlobalConfigArgs,

    #[clap(index = 1, required = true)]
    source: PathBuf,
}

impl ImportGame {
    fn run(self) -> anyhow::Result<()> {
        let project = self.env.load_project()?;
        let game_path = project.game_path()?;
        let project_manifest = project.manifest_opt()?;

        anyhow::ensure!(
            std::fs::read_dir(game_path)?.next().is_none(),
            "Project game directory has files that would be overwritten."
        );

        let files_to_copy: Vec<_> = if let Some(project_manifest) = project_manifest {
            let validation_result = validate_manifest_in_game_dir(&self.source, project_manifest)?;
            anyhow::ensure!(
                validation_result.missing.is_empty(),
                "Some files are missing in source directory: {:?}",
                validation_result.missing
            );

            anyhow::ensure!(
                validation_result.mismatches.is_empty(),
                "Some files in the source directory do not match the project manifest: {:?}",
                validation_result.mismatches
            );

            validation_result.matches.into_iter().collect()
        } else {
            RelWalkDir::new(&self.source).collect::<anyhow::Result<_>>()?
        };

        for path in files_to_copy {
            eprintln!("Copying {path}...");
            let dest = game_path.join(&path);
            std::fs::create_dir_all(dest.parent().unwrap())?;
            std::fs::copy(self.source.join(&path), &dest)?;
        }

        if project_manifest.is_none() {
            let manifest = generate_manifest_from_game_dir(game_path)?;
            let manifest_path = project.manifest_path()?;
            std::fs::write(manifest_path, serde_json::to_string_pretty(&manifest)?)?;
        }

        Ok(())
    }
}

#[derive(ValueEnum, Clone, Debug)]
enum RequiredTool {
    #[clap(name = "espeak")]
    EpeakNG,
    #[clap(name = "ffmpeg")]
    Ffmpeg,
    #[clap(name = "scinc")]
    Scinc,
}

/// Checks that the distribution environment is properly set up.
/// Should generally not be necessary outside of building for distribution.
#[derive(Debug, Parser)]
struct CheckDistribution {
    #[clap(long)]
    test_fail: bool,

    #[clap(long, value_delimiter = ',')]
    required_tools: Vec<RequiredTool>,
}

impl CheckDistribution {
    fn run(self) -> anyhow::Result<()> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        rt.block_on(async move {
            let mut is_valid = true;
            let env = DistEnv::try_load_env()?;
            for req_tool in &self.required_tools {
                match req_tool {
                    RequiredTool::EpeakNG => {
                        if let Some(espeak) = env.espeak_tool() {
                            eprintln!("espeak binary found. Execution params: {espeak:?}");
                            let output = espeak.cmd_async().arg("--version").output().await?;
                            if output.status.success() {
                                eprintln!(
                                    "espeak-ng found. Version info:\n{}",
                                    String::from_utf8_lossy(&output.stdout)
                                );
                            } else {
                                is_valid = false;
                                eprintln!(
                                    "Unable to run espeak-ng: {}",
                                    String::from_utf8_lossy(&output.stderr)
                                );
                            }
                        } else {
                            is_valid = false;
                            eprintln!("espeak-ng not found.");
                        }
                    }
                    RequiredTool::Ffmpeg => {
                        let ffmpeg = env.ffmpeg_tool();
                        eprintln!("ffmpeg binary found. Execution params: {ffmpeg:?}");
                        let output = ffmpeg.cmd_async().arg("-version").output().await?;
                        if output.status.success() {
                            eprintln!(
                                "ffmpeg found. Version info:\n{}",
                                String::from_utf8_lossy(&output.stdout)
                            );
                        } else {
                            is_valid = false;
                            eprintln!(
                                "Unable to run ffmpeg: {}",
                                String::from_utf8_lossy(&output.stderr)
                            );
                        }
                    }
                    RequiredTool::Scinc => {
                        if let Some(scinc) = env.scinc_tool() {
                            eprintln!("scinc binary found. Execution params: {scinc:?}");
                            let output = scinc.cmd_async().arg("--version").output().await?;
                            if output.status.success() {
                                eprintln!(
                                    "scinc found. Version info:\n{}",
                                    String::from_utf8_lossy(&output.stdout)
                                );
                            } else {
                                is_valid = false;
                                eprintln!(
                                    "Unable to run scinc: {}",
                                    String::from_utf8_lossy(&output.stderr)
                                );
                            }
                        } else {
                            is_valid = false;
                            eprintln!("scinc not found.");
                        }
                    }
                }
            }

            if self.test_fail {
                is_valid = false;
                eprintln!("Testing check fail");
            }

            anyhow::ensure!(is_valid, "Distribution environment is not properly set up.");
            Ok(())
        })
    }
}
