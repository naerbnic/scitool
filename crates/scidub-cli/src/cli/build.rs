use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    pin::Pin,
};

use clap::Parser;
use csv::WriterBuilder;
use indicatif::ProgressFinish;
use scidev::ids::LineId;
use sciproj::{
    build::audio::{ProgressFactory, compile_audio_base},
    path::{
        abspath::AbsPath,
        relpath::{RelPath, RelPathBuf},
        segment::Segment,
    },
    resources::AudioClip,
    scripts::SciScriptExports,
    tools::scinc::{CompileEnv, CompilerInputs, SciVersion},
};
use serde::{Deserialize, Serialize};

use crate::{
    cli::GlobalConfigArgs,
    data::{ToFromStringSerde, load_data},
    dist_env::DistEnv,
    project::Project,
    rt::run_async,
    walkdir::RelWalkDir,
};

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

const SCI_SCRIPT_EXT: &str = "sc";

/// Builds a resource patch from a target.
#[derive(Debug, Parser)]
pub(super) struct Build {
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
    fn find_target_info(&self, project: &Project) -> anyhow::Result<(PathBuf, &str)> {
        let (line_mapping, target_name) = if let Some(target) = &self.target {
            let Some(target_config) = project.config()?.targets().get(target) else {
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
        Ok((line_mapping, target_name))
    }

    pub(super) fn run(self) -> anyhow::Result<()> {
        let project = self.env.load_project()?;

        let config = project.config()?;
        let book = project.book()?;

        let (line_mapping_path, target_name) = self.find_target_info(project)?;

        let line_mappings: Vec<LineMapping> =
            load_data(&line_mapping_path, &crate::data::DataFormat::Csv)?;

        let audio_files_root = self.audio_files.clone().unwrap_or_else(|| {
            config
                .audio_files_root()
                .unwrap_or(RelPath::EMPTY)
                .to_std_path(project.root())
        });

        let clip_map = line_mappings_to_clip_map(line_mappings, &audio_files_root)?;

        let target_relpath = Segment::try_new(target_name)
            .ok_or_else(|| anyhow::anyhow!("Target name must be a valid relative path segment"))?;

        let base_output_dir = if let Some(output_dir) = &self.output {
            output_dir.clone()
        } else {
            project.build_dir()?.join(target_relpath)
        };

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
        std::fs::create_dir_all(&base_output_dir)?;

        let res_dir_path = base_output_dir.join("res");
        std::fs::create_dir_all(&res_dir_path)?;

        let stderr_term = console::Term::buffered_stderr();
        let progress_factory =
            ProgressFactory::new(stderr_term).with_finish(ProgressFinish::AndLeave);

        let dist_env = DistEnv::from_env();

        let ffmpeg_tool = dist_env
            .ffmpeg_tool()?
            .ok_or_else(|| anyhow::anyhow!("Couldn't find the ffmpeg binary"))?;
        let espeak_tool = dist_env.espeak_tool()?;

        run_async(async {
            let mut script_build_step: Vec<Pin<Box<dyn Future<Output = anyhow::Result<()>>>>> =
                Vec::new();
            script_build_step.push(Box::pin(compile_audio_base(
                &*ffmpeg_tool,
                espeak_tool.as_deref(),
                progress_factory,
                book,
                &clip_map,
                &res_dir_path,
            )));

            if let Some(scripts_dir) = project.scripts_dir_opt()? {
                script_build_step.push(Box::pin(self.build_scripts(
                    dist_env,
                    project,
                    scripts_dir,
                    AbsPath::from_std(&res_dir_path),
                )));
            }

            futures_util::future::join_all(script_build_step)
                .await
                .into_iter()
                .collect::<Result<Vec<_>, anyhow::Error>>()?;
            Ok(())
        })
    }

    async fn build_scripts(
        &self,
        env: &DistEnv,
        proj: &Project,
        scripts_dir: &AbsPath,
        out_res_dir: &AbsPath,
    ) -> anyhow::Result<()> {
        let source_config = proj.config()?.source_config();
        let header_exports = SciScriptExports::read_from_resources(
            proj.resources_opt()?
                .ok_or_else(|| anyhow::anyhow!("building scripts requires resources."))?,
        )?;

        let mut source_files = Vec::new();

        for entry in RelWalkDir::new(scripts_dir) {
            let entry = entry?;
            if let Some(ext) = entry.extension()
                && ext == SCI_SCRIPT_EXT
            {
                source_files.push(scripts_dir.join_rel(entry));
            }
        }

        if source_files.is_empty() {
            eprintln!("No script files found in {scripts_dir}");
            return Ok(());
        }

        let scinc_tool = env
            .scinc_tool()?
            .ok_or_else(|| anyhow::anyhow!("Must have scinc available to build scripts."))?;

        let global_includes = source_config
            .global_includes()
            .into_iter()
            .map(|p| scripts_dir.join_rel(p))
            .collect();

        let include_dirs = source_config
            .include_paths()
            .into_iter()
            .map(|p| scripts_dir.join_rel(p))
            .collect();

        let compile_env = CompileEnv {
            class_defs: header_exports.class_defs_header().to_string(),
            selectors: header_exports.selectors_header().to_string(),
        };

        let compile_inputs = CompilerInputs {
            sci_version: SciVersion::V1_1,
            global_includes,
            include_dirs,
            source_files,
        };

        scinc_tool
            .compile_scripts(&compile_env, &compile_inputs, out_res_dir)
            .await?;
        Ok(())
    }
}
