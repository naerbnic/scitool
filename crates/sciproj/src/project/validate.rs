#![expect(dead_code)]

use std::path::Path;

pub(super) fn validate_relative_path(path: &impl AsRef<Path>) -> bool {
    let path = path.as_ref();
    for component in path.components() {
        match component {
            std::path::Component::Normal(_) => {}
            std::path::Component::Prefix(_)
            | std::path::Component::RootDir
            | std::path::Component::CurDir
            | std::path::Component::ParentDir => return false,
        }
    }
    true
}
