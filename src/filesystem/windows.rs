use std::path::Path;
use std::os::windows::ffi::OsStrExt;
use crate::filesystem::provider::{FileSystemProvider, FsError, DirEntryInfo, FileMetadata};

pub struct WindowsFileSystemProvider;

const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x00000400;

fn get_file_attributes(path: &Path) -> Option<u32> {
    use windows_sys::Win32::Storage::FileSystem::GetFileAttributesW;
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let attrs = unsafe { GetFileAttributesW(wide.as_ptr()) };
    if attrs == u32::MAX {
        None
    } else {
        Some(attrs)
    }
}

impl FileSystemProvider for WindowsFileSystemProvider {
    fn list_directory(&self, path: &Path) -> Result<Vec<DirEntryInfo>, FsError> {
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            let metadata = entry.metadata()?;
            entries.push(DirEntryInfo {
                name,
                is_dir: metadata.is_dir(),
                size: metadata.len(),
                modified: metadata.modified().ok(),
            });
        }
        Ok(entries)
    }

    fn metadata(&self, path: &Path) -> Result<FileMetadata, FsError> {
        let m = std::fs::symlink_metadata(path)
            .map_err(FsError::Io)?;
        let is_reparse = self.detect_reparse_point(path).unwrap_or(false);
        Ok(FileMetadata {
            is_dir: m.is_dir(),
            size: m.len(),
            modified: m.modified().ok(),
            is_reparse_point: is_reparse,
        })
    }

    fn read_file(&self, path: &Path) -> Result<std::fs::File, FsError> {
        Ok(std::fs::File::open(path)?)
    }

    fn write_file(&self, path: &Path, data: &[u8]) -> Result<(), FsError> {
        std::fs::write(path, data)?;
        Ok(())
    }

    fn mkdir(&self, path: &Path) -> Result<(), FsError> {
        std::fs::create_dir(path)?;
        Ok(())
    }

    fn rename(&self, from: &Path, to: &Path) -> Result<(), FsError> {
        std::fs::rename(from, to)?;
        Ok(())
    }

    fn delete(&self, path: &Path) -> Result<(), FsError> {
        if path.is_dir() {
            std::fs::remove_dir_all(path)?;
        } else {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }

    fn detect_reparse_point(&self, path: &Path) -> Result<bool, FsError> {
        match get_file_attributes(path) {
            Some(attrs) => Ok((attrs & FILE_ATTRIBUTE_REPARSE_POINT) != 0),
            None => {
                let m = std::fs::symlink_metadata(path)?;
                Ok(m.file_type().is_symlink())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[test]
    fn windows_detect_reparse_for_regular_file_is_false() {
        let p = WindowsFileSystemProvider;
        let tmp = temp_dir();
        let is_reparse = p.detect_reparse_point(&tmp).unwrap_or(false);
        assert_eq!(is_reparse, false);
    }
}