use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FsError {
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),
    #[error("路径未找到: {0}")]
    NotFound(String),
    #[error("权限不足: {0}")]
    PermissionDenied(String),
    #[error("检测到重解析点（符号链接/junction），拒绝访问: {0}")]
    ReparsePointDenied(String),
    #[error("路径超出共享目录范围: {0}")]
    PathOutsideShare(String),
}

impl actix_web::ResponseError for FsError {
    fn status_code(&self) -> actix_web::http::StatusCode {
        use actix_web::http::StatusCode;
        match self {
            FsError::NotFound(_) => StatusCode::NOT_FOUND,
            FsError::PermissionDenied(_) => StatusCode::FORBIDDEN,
            FsError::ReparsePointDenied(_) => StatusCode::FORBIDDEN,
            FsError::PathOutsideShare(_) => StatusCode::FORBIDDEN,
            FsError::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DirEntryInfo {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: Option<std::time::SystemTime>,
}

#[derive(Debug, Clone)]
pub struct FileMetadata {
    pub is_dir: bool,
    pub size: u64,
    pub modified: Option<std::time::SystemTime>,
    pub is_reparse_point: bool,
}

pub trait FileSystemProvider: Send + Sync {
    fn list_directory(&self, path: &Path) -> Result<Vec<DirEntryInfo>, FsError>;
    fn metadata(&self, path: &Path) -> Result<FileMetadata, FsError>;
    fn read_file(&self, path: &Path) -> Result<std::fs::File, FsError>;
    fn write_file(&self, path: &Path, data: &[u8]) -> Result<(), FsError>;
    fn mkdir(&self, path: &Path) -> Result<(), FsError>;
    fn rename(&self, from: &Path, to: &Path) -> Result<(), FsError>;
    fn delete(&self, path: &Path) -> Result<(), FsError>;
    fn detect_reparse_point(&self, path: &Path) -> Result<bool, FsError>;
}