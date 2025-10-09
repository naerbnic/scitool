#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileType {
    inner: cap_std::fs::FileType,
}

impl FileType {
    pub(super) fn of_cap_std(ft: cap_std::fs::FileType) -> Self {
        FileType { inner: ft }
    }

    #[must_use]
    pub fn is_dir(&self) -> bool {
        self.inner.is_dir()
    }

    #[must_use]
    pub fn is_file(&self) -> bool {
        !self.inner.is_dir()
    }
}
