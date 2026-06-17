use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use anyhow::Context;
use clap::{Parser, Subcommand};
use sciproj::path::relpath::{RelPath, RelPathBuf};

use crate::{cli::GlobalConfigArgs, project::Manifest, walkdir::RelWalkDir};

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

/// Manage game resource data.
#[derive(Debug, Parser)]
pub(super) struct GameData {
    #[clap(subcommand)]
    command: GameDataSubcommand,
}

impl GameData {
    pub(super) fn run(self) -> anyhow::Result<()> {
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
        let manifest = Manifest::generate_from_game_dir(game_path)?;

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
            let manifest = Manifest::generate_from_game_dir(game_path)?;
            let manifest_path = project.manifest_path()?;
            std::fs::write(manifest_path, serde_json::to_string_pretty(&manifest)?)?;
        }

        Ok(())
    }
}
