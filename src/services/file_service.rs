use chrono::TimeZone;
use std::path::Path;
use tokio::fs;
use crate::error::AppError;
use crate::models::file_info::{FileInfo, DirectoryTreeNode};
use crate::utils::path_sanitizer::{sanitize_path, validate_filename};
use crate::utils::file_type::{get_mime_type, get_preview_type, PreviewType};

pub struct FileService {
    pub base_dir: String,
}

impl FileService {
    pub fn new(base_dir: &str) -> Self {
        Self { base_dir: base_dir.to_string() }
    }

    pub async fn list_directory(
        &self,
        path: &str,
        show_hidden: bool,
        sort_by: &str,
        sort_order: &str,
    ) -> Result<Vec<FileInfo>, AppError> {
        let safe_path = sanitize_path(path, &self.base_dir)
            .map_err(|e| AppError::PathTraversal(e))?;

        let full_path = Path::new(&safe_path);

        if !full_path.is_dir() {
            return Err(AppError::NotFound("目录不存在".to_string()));
        }

        let mut entries: Vec<FileInfo> = Vec::new();
        let mut read_dir = fs::read_dir(&full_path).await
            .map_err(|e| AppError::Internal(format!("读取目录失败: {}", e)))?;

        while let Some(entry) = read_dir.next_entry().await
            .map_err(|e| AppError::Internal(format!("读取目录条目失败: {}", e)))? {
            let file_name = entry.file_name().to_string_lossy().to_string();
            if !show_hidden && file_name.starts_with('.') {
                continue;
            }
            if file_name == "lost+found" { continue; }

            let metadata = entry.metadata().await
                .map_err(|e| AppError::Internal(format!("读取文件元数据失败: {}", e)))?;
            let is_dir = metadata.is_dir();
            let size = metadata.len();
            let modified = metadata.modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| {
                    chrono::Local.timestamp(d.as_secs() as i64, 0)
                        .format("%Y-%m-%d %H:%M:%S")
                        .to_string()
                })
                .unwrap_or_default();

            let extension = Path::new(&file_name)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");

            let (mime_type, preview_type) = if is_dir {
                (None, None)
            } else {
                (Some(get_mime_type(extension)), Some(get_preview_type(extension).as_str().to_string()))
            };

            let relative_path = safe_path.trim_start_matches(&self.base_dir).trim_start_matches('/');
            let entry_path = if relative_path.is_empty() {
                format!("/{}", file_name)
            } else {
                format!("/{}/{}", relative_path, file_name)
            };

            entries.push(FileInfo {
                name: file_name,
                path: entry_path,
                is_dir,
                size,
                modified,
                mime_type,
                preview_type,
            });
        }

        entries.sort_by(|a, b| {
            let cmp = match sort_by {
                "name" => a.name.cmp(&b.name),
                "size" => a.size.cmp(&b.size),
                "modified" => a.modified.cmp(&b.modified),
                "type" => {
                    let a_type = if a.is_dir { "dir" } else { "file" };
                    let b_type = if b.is_dir { "dir" } else { "file" };
                    a_type.cmp(b_type)
                }
                _ => a.name.cmp(&b.name),
            };
            if sort_order == "desc" { cmp.reverse() } else { cmp }
        });

        Ok(entries)
    }

    pub async fn get_tree(&self, path: &str) -> Result<Vec<DirectoryTreeNode>, AppError> {
        let safe_path = sanitize_path(path, &self.base_dir)
            .map_err(|e| AppError::PathTraversal(e))?;

        let full_path = Path::new(&safe_path);
        if !full_path.is_dir() {
            return Err(AppError::NotFound("目录不存在".to_string()));
        }

        let mut nodes = Vec::new();
        let mut read_dir = fs::read_dir(&full_path).await
            .map_err(|e| AppError::Internal(format!("读取目录失败: {}", e)))?;

        while let Some(entry) = read_dir.next_entry().await
            .map_err(|e| AppError::Internal(format!("读取目录条目失败: {}", e)))? {
            let file_name = entry.file_name().to_string_lossy().to_string();
            if file_name.starts_with('.') { continue; }
            if file_name == "lost+found" { continue; }
            if file_name == "lost+found" { continue; }

            let metadata = entry.metadata().await
                .map_err(|e| AppError::Internal(format!("读取元数据失败: {}", e)))?;
            if !metadata.is_dir() { continue; }

            let entry_path = entry.path();
            let has_children = self.has_subdirectories(&entry_path).await;

            let relative_path = entry_path.to_string_lossy().trim_start_matches(&self.base_dir).to_string();

            nodes.push(DirectoryTreeNode {
                path: relative_path,
                name: file_name,
                has_children,
                expanded: false,
                children: Vec::new(),
            });
        }

        nodes.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(nodes)
    }

    async fn has_subdirectories(&self, dir_path: &Path) -> bool {
        if let Ok(mut read_dir) = fs::read_dir(dir_path).await {
            while let Ok(Some(entry)) = read_dir.next_entry().await {
                if let Ok(metadata) = entry.metadata().await {
                    if metadata.is_dir() {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub async fn search(&self, keyword: &str, path: &str, max_depth: u32) -> Result<Vec<FileInfo>, AppError> {
        let safe_path = sanitize_path(path, &self.base_dir)
            .map_err(|e| AppError::PathTraversal(e))?;
        let mut results = Vec::new();
        self.search_recursive(keyword, Path::new(&safe_path), &self.base_dir, 0, max_depth, &mut results).await?;
        Ok(results)
    }

    async fn search_recursive(
        &self,
        keyword: &str,
        dir: &Path,
        base_dir: &str,
        current_depth: u32,
        max_depth: u32,
        results: &mut Vec<FileInfo>,
    ) -> Result<(), AppError> {
        if current_depth > max_depth { return Ok(()); }

        let mut read_dir = fs::read_dir(dir).await
            .map_err(|e| AppError::Internal(format!("搜索读取目录失败: {}", e)))?;

        while let Some(entry) = read_dir.next_entry().await
            .map_err(|e| AppError::Internal(format!("搜索读取条目失败: {}", e)))? {
            let file_name = entry.file_name().to_string_lossy().to_string();
            if file_name.starts_with('.') { continue; }

            let metadata = entry.metadata().await
                .map_err(|e| AppError::Internal(format!("搜索读取元数据失败: {}", e)))?;
            let is_dir = metadata.is_dir();
            let entry_path = entry.path();

            if file_name.to_lowercase().contains(&keyword.to_lowercase()) {
                let relative_path = entry_path.to_string_lossy().trim_start_matches(base_dir).to_string();
                results.push(FileInfo {
                    name: file_name.clone(),
                    path: relative_path,
                    is_dir,
                    size: metadata.len(),
                    modified: metadata.modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| {
                            chrono::Local.timestamp(d.as_secs() as i64, 0)
                                .format("%Y-%m-%d %H:%M:%S")
                                .to_string()
                        })
                        .unwrap_or_default(),
                    mime_type: None,
                    preview_type: None,
                });
            }

            if is_dir {
                Box::pin(self.search_recursive(keyword, &entry_path, base_dir, current_depth + 1, max_depth, results)).await?;
            }
        }
        Ok(())
    }

    pub async fn mkdir(&self, parent_path: &str, name: &str) -> Result<(), AppError> {
        validate_filename(name).map_err(|e| AppError::BadRequest(e))?;
        let safe_parent = sanitize_path(parent_path, &self.base_dir)
            .map_err(|e| AppError::PathTraversal(e))?;
        let full_path = Path::new(&safe_parent).join(name);
        if full_path.exists() {
            return Err(AppError::Conflict("文件夹已存在".to_string()));
        }
        fs::create_dir(&full_path).await
            .map_err(|e| AppError::Internal(format!("创建文件夹失败: {}", e)))?;
        Ok(())
    }

    pub async fn rename(&self, path: &str, new_name: &str) -> Result<(), AppError> {
        validate_filename(new_name).map_err(|e| AppError::BadRequest(e))?;
        let safe_path = sanitize_path(path, &self.base_dir)
            .map_err(|e| AppError::PathTraversal(e))?;
        let old_path = Path::new(&safe_path);
        let parent = old_path.parent().ok_or_else(|| AppError::BadRequest("无效路径".to_string()))?;
        let new_path = parent.join(new_name);
        if new_path.exists() {
            return Err(AppError::Conflict("目标名称已存在".to_string()));
        }
        fs::rename(&old_path, &new_path).await
            .map_err(|e| AppError::Internal(format!("重命名失败: {}", e)))?;
        Ok(())
    }

    pub async fn delete(&self, path: &str) -> Result<(), AppError> {
        let safe_path = sanitize_path(path, &self.base_dir)
            .map_err(|e| AppError::PathTraversal(e))?;
        let full_path = Path::new(&safe_path);
        if full_path.is_dir() {
            fs::remove_dir_all(&full_path).await
                .map_err(|e| AppError::Internal(format!("删除目录失败: {}", e)))?;
        } else {
            fs::remove_file(&full_path).await
                .map_err(|e| AppError::Internal(format!("删除文件失败: {}", e)))?;
        }
        Ok(())
    }

    pub async fn move_file(&self, source_path: &str, target_dir: &str) -> Result<(), AppError> {
        let safe_source = sanitize_path(source_path, &self.base_dir)
            .map_err(|e| AppError::PathTraversal(e))?;
        let safe_target = sanitize_path(target_dir, &self.base_dir)
            .map_err(|e| AppError::PathTraversal(e))?;
        let source = Path::new(&safe_source);
        let file_name = source.file_name().ok_or_else(|| AppError::BadRequest("无效源路径".to_string()))?;
        let target = Path::new(&safe_target).join(file_name);
        fs::rename(&source, &target).await
            .map_err(|e| AppError::Internal(format!("移动失败: {}", e)))?;
        Ok(())
    }

    pub async fn copy_file(&self, source_path: &str, target_dir: &str) -> Result<(), AppError> {
        let safe_source = sanitize_path(source_path, &self.base_dir)
            .map_err(|e| AppError::PathTraversal(e))?;
        let safe_target = sanitize_path(target_dir, &self.base_dir)
            .map_err(|e| AppError::PathTraversal(e))?;
        let source = Path::new(&safe_source);
        let file_name = source.file_name().ok_or_else(|| AppError::BadRequest("无效源路径".to_string()))?;
        let target = Path::new(&safe_target).join(file_name);
        fs::copy(&source, &target).await
            .map_err(|e| AppError::Internal(format!("复制失败: {}", e)))?;
        Ok(())
    }
}

impl PreviewType {
    pub fn as_str(&self) -> &str {
        match self {
            PreviewType::Image => "image",
            PreviewType::Text => "text",
            PreviewType::Pdf => "pdf",
            PreviewType::Video => "video",
            PreviewType::Audio => "audio",
            PreviewType::None => "none",
        }
    }
}
