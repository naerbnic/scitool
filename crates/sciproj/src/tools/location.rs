use crate::path::{
    abspath::{AbsPath, AbsPathBuf},
    relpath::RelPathBuf,
};

#[derive(Debug, Clone)]
pub struct SystemPathLocation {
    bin_path: AbsPathBuf,
}

impl SystemPathLocation {
    #[must_use]
    pub fn new(bin_path: AbsPathBuf) -> Self {
        Self { bin_path }
    }

    #[must_use]
    pub fn bin_path(&self) -> &AbsPath {
        &self.bin_path
    }
}

#[derive(Debug, Clone)]
pub struct DistLocation {
    install_root: AbsPathBuf,
    bin_path: RelPathBuf,
}

impl DistLocation {
    #[must_use]
    pub fn new(install_root: AbsPathBuf, bin_path: RelPathBuf) -> Self {
        Self {
            install_root,
            bin_path,
        }
    }

    #[must_use]
    pub fn bin_path(&self) -> AbsPathBuf {
        self.install_root.join_rel(&self.bin_path)
    }

    #[must_use]
    pub fn install_root(&self) -> &AbsPath {
        &self.install_root
    }
}

#[derive(Debug, Clone)]
pub enum ToolLocation {
    System(SystemPathLocation),
    Dist(DistLocation),
}

impl From<SystemPathLocation> for ToolLocation {
    fn from(value: SystemPathLocation) -> Self {
        ToolLocation::System(value)
    }
}

impl From<DistLocation> for ToolLocation {
    fn from(value: DistLocation) -> Self {
        ToolLocation::Dist(value)
    }
}

impl ToolLocation {
    #[must_use]
    pub fn bin_path(&self) -> AbsPathBuf {
        match self {
            ToolLocation::System(loc) => loc.bin_path().to_buf(),
            ToolLocation::Dist(dist_location) => dist_location.bin_path(),
        }
    }
}
