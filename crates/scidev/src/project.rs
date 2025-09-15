//! Defines the concept of a scidev project, which is an expansion of the data
//! resource file concept to include multiple files and other metadata.

pub mod schema;

pub struct Project {
    #[expect(dead_code, reason = "Will use to export data")]
    data: schema::Project,
}
