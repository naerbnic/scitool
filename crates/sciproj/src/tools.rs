use std::{fmt::Debug, pin::Pin, process::Output};

use crate::tools::location::ToolLocation;

pub mod espeak;
pub mod ffmpeg;
pub mod location;
pub mod scinc;

mod util;

pub trait TestableTool: Debug {
    fn test_binary<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<Output>> + Send + 'a>>;
}

pub trait ToolSearcher {
    fn find_tool_locations(&self, name: &str) -> anyhow::Result<Vec<ToolLocation>>;
}
