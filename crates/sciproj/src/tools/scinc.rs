use std::process::Output;

use tokio::process::Command;

use crate::{
    path::{
        abspath::{AbsPath, AbsPathBuf},
        relpath::RelPath,
    },
    tools::{TestableTool, location::DistLocation},
};

#[derive(Debug)]
pub struct CompileEnv {
    class_defs: String,
    selectors: String,
}

#[derive(Debug)]
pub enum SciVersion {
    V1_1,
}

impl SciVersion {
    fn version_path(&self) -> &RelPath {
        match self {
            Self::V1_1 => RelPath::from_static("sci1_1"),
        }
    }

    fn version_arg(&self) -> &str {
        match self {
            Self::V1_1 => "SCI_1_1",
        }
    }
}

#[derive(Debug)]
pub struct CompilerInputs {
    sci_version: SciVersion,
    global_includes: Vec<AbsPathBuf>,
    include_dirs: Vec<AbsPathBuf>,
    source_files: Vec<AbsPathBuf>,
}

pub trait Scinc: TestableTool {
    fn compile_scripts<'a>(
        &'a self,
        env: &'a CompileEnv,
        inputs: &'a CompilerInputs,
        output_dir: &'a AbsPath,
    ) -> Box<dyn Future<Output = anyhow::Result<()>> + Send + 'a>;
}

const INCLUDE_REL_PATH: &RelPath = RelPath::from_static("include");

pub type BoxScinc = Box<dyn Scinc>;

pub fn from_dist_location(dist: &DistLocation) -> anyhow::Result<Box<dyn Scinc>> {
    Ok(Box::new(ScincTool::from_dist(dist)))
}

#[derive(Debug)]
struct ScincTool {
    bin_path: AbsPathBuf,
    includes_root: AbsPathBuf,
}

impl ScincTool {
    fn from_dist(dist: &DistLocation) -> Self {
        Self {
            bin_path: dist.bin_path(),
            includes_root: dist.install_root().join_rel(INCLUDE_REL_PATH).to_buf(),
        }
    }
}

impl ScincTool {
    async fn compile_scripts_impl(
        &self,
        env: &CompileEnv,
        inputs: &CompilerInputs,
        output_dir: &AbsPath,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            output_dir.is_dir(),
            "expected directory for output: {output_dir}"
        );

        let tempdir = tempfile::tempdir()?;
        let tempdir_path = AbsPath::from_std(tempdir.path());

        let classdef_file_path = tempdir_path.join("class_defs.sh");
        let selector_file_path = tempdir_path.join("selectors.sh");

        tokio::fs::write(&classdef_file_path, &env.class_defs).await?;
        tokio::fs::write(&selector_file_path, &env.selectors).await?;

        let system_include_path = self
            .includes_root
            .join_rel(inputs.sci_version.version_path());
        let system_header = system_include_path.join("system.sh");
        let mut include_paths: Vec<&AbsPath> = vec![&system_include_path];
        for input_inc_path in &inputs.include_dirs {
            include_paths.push(input_inc_path);
        }

        let mut global_includes: Vec<&AbsPath> = vec![&system_header];
        for input_inc_path in &inputs.global_includes {
            global_includes.push(input_inc_path);
        }

        let mut source_paths: Vec<&AbsPath> = Vec::new();
        for input_src_path in &inputs.source_files {
            source_paths.push(input_src_path);
        }

        let mut cmd = Command::new(&self.bin_path);
        cmd.arg("-u")
            .arg("-a")
            .args(["-t", inputs.sci_version.version_arg()])
            .args(["-o", output_dir.as_str()])
            .args(["--selector_file", selector_file_path.as_str()])
            .args(["--classdef_file", classdef_file_path.as_str()]);

        for include_path in include_paths {
            cmd.args(["--include_path", include_path.as_str()]);
        }
        for global_include in global_includes {
            cmd.args(["-global_include", global_include.as_str()]);
        }
        for src_path in source_paths {
            cmd.arg(src_path);
        }

        let mut child = cmd.spawn()?;

        let status = child.wait().await?;

        anyhow::ensure!(status.success(), "scinc failed");

        Ok(())
    }

    async fn test_binary_impl(&self) -> anyhow::Result<Output> {
        Ok(Command::new(&self.bin_path)
            .arg("--version")
            .output()
            .await?)
    }
}

impl TestableTool for ScincTool {
    fn test_binary<'a>(
        &'a self,
    ) -> std::pin::Pin<Box<dyn Future<Output = anyhow::Result<Output>> + Send + 'a>> {
        Box::pin(self.test_binary_impl())
    }
}

impl Scinc for ScincTool {
    fn compile_scripts<'a>(
        &'a self,
        env: &'a CompileEnv,
        inputs: &'a CompilerInputs,
        output_dir: &'a AbsPath,
    ) -> Box<dyn Future<Output = anyhow::Result<()>> + Send + 'a> {
        Box::new(self.compile_scripts_impl(env, inputs, output_dir))
    }
}
