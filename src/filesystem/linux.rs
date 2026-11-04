use std::path::Path;
use crate::filesystem::provider::{FileSystemProvider, FsError, DirEntryInfo, FileMetadata};

pub struct LinuxFileSystemProvider;

impl FileSystemProvider for LinuxFileSystemProvider {
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
        let m = std::fs::metadata(path)?;
        Ok(FileMetadata {
            is_dir: m.is_dir(),
            size: m.len(),
            modified: m.modified().ok(),
            is_reparse_point: false,
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

    fn detect_reparse_point(&self, _path: &Path) -> Result<bool, FsError> {
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[test]
    fn linux_detect_reparse_always_false() {
        let p = LinuxFileSystemProvider;
        let tmp = temp_dir();
        assert_eq!(p.detect_reparse_point(&tmp).unwrap(), false);
    }
}