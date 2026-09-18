use scidev_inc::ArtifactKey;
use sciproj::path::abspath::AbsPathBuf;

#[derive(Hash, Eq, PartialEq, Debug)]
pub(super) struct Root;

impl ArtifactKey for Root {
    type Value = AbsPathBuf;
}
