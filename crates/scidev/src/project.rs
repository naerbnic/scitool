//! Defines the concept of a scidev project, which is an expansion of the data
//! resource file concept to include multiple files and other metadata.

use crate::resources::file::ResourceSet;

pub mod schema;

pub struct Project {
    data: schema::Project,
}

impl Project {
    /// Load a project from a JSON string.
    pub fn export_from_resources(resources: &ResourceSet) -> anyhow::Result<Self> {
        todo!();
    }
}
