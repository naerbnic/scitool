mod unblock;

use crate::executor::Handle;

pub(crate) struct StdProvider {
    handle: Handle,
}

impl super::SyncFile for std::fs::File {}

// struct AsyncWrap {
//     file: std::fs::File,
//     handle: Handle,
// }

// impl async_io::AsyncRead for AsyncWrap {
//     fn poll_read(
//         self: std::pin::Pin<&mut Self>,
//         cx: &mut std::task::Context<'_>,
//         buf: &mut async_io::ReadBuf<'_>,
//     ) -> std::task::Poll<std::io::Result<()>> {
//         todo!()
//     }
// }

// impl super::VfsProvider for StdProvider {
//     type FileHandle = std::fs::File;

//     type DirHandle = cap_std::fs::Dir;

//     type SyncFile = std::fs::File;

//     type AsyncFile;

//     fn open_file_handle(
//         &self,
//         path: scidev_path::relpath::RelPath,
//         open_options: super::OpenOptions,
//     ) -> crate::promise::Promise<std::io::Result<Self::FileHandle>> {
//         todo!()
//     }

//     fn open_dir_handle(
//         &self,
//         path: scidev_path::relpath::RelPath,
//     ) -> crate::promise::Promise<std::io::Result<Self::DirHandle>> {
//         todo!()
//     }

//     fn get_file_metadata(
//         &self,
//         file: &Self::FileHandle,
//     ) -> crate::promise::Promise<std::io::Result<super::Metadata>> {
//         todo!()
//     }

//     fn set_file_length(
//         &self,
//         file: &Self::FileHandle,
//         size: u64,
//     ) -> crate::promise::Promise<std::io::Result<()>> {
//         todo!()
//     }

//     fn set_file_times(
//         &self,
//         file: &Self::FileHandle,
//         times: &std::fs::FileTimes,
//     ) -> crate::promise::Promise<std::io::Result<()>> {
//         todo!()
//     }

//     fn sync_file(&self, file: &Self::FileHandle) -> crate::promise::Promise<std::io::Result<()>> {
//         todo!()
//     }

//     fn delete_file(&self, file: &Self::FileHandle) -> crate::promise::Promise<std::io::Result<()>> {
//         todo!()
//     }

//     fn open_sync_file_io(
//         &self,
//         file: Self::FileHandle,
//     ) -> crate::promise::Promise<std::io::Result<Self::SyncFile>> {
//         todo!()
//     }

//     fn open_async_file_io(
//         &self,
//         file: Self::FileHandle,
//     ) -> crate::promise::Promise<std::io::Result<Self::AsyncFile>> {
//         todo!()
//     }

//     fn read_dir(
//         &self,
//         dir: &Self::DirHandle,
//     ) -> crate::promise::PromiseStream<std::io::Result<super::DirEntry>> {
//         todo!()
//     }

//     fn make_dir(
//         &self,
//         base: &Self::DirHandle,
//         name: &scidev_path::segment::Segment,
//     ) -> crate::promise::Promise<std::io::Result<Self::DirHandle>> {
//         todo!()
//     }

//     fn delete_dir(&self, dir: &Self::DirHandle) -> crate::promise::Promise<std::io::Result<()>> {
//         todo!()
//     }
// }
