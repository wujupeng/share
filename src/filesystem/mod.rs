pub mod provider;
pub mod linux;
#[cfg(target_os = "windows")]
pub mod windows;

pub use provider::{FileSystemProvider, FsError, FileMetadata, DirEntryInfo};

#[cfg(target_os = "linux")]
pub fn default_provider() -> Box<dyn FileSystemProvider> {
    Box::new(linux::LinuxFileSystemProvider)
}

#[cfg(target_os = "windows")]
pub fn default_provider() -> Box<dyn FileSystemProvider> {
    Box::new(windows::WindowsFileSystemProvider)
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
pub fn default_provider() -> Box<dyn FileSystemProvider> {
    Box::new(linux::LinuxFileSystemProvider)
}