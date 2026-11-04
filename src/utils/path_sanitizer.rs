use std::path::PathBuf;
use crate::filesystem::default_provider;

#[cfg(target_os = "windows")]
fn is_path_separator(ch: char) -> bool {
    ch == '/' || ch == '\\'
}

#[cfg(not(target_os = "windows"))]
fn is_path_separator(ch: char) -> bool {
    ch == '/'
}

pub fn sanitize_path(path: &str, base_dir: &str) -> Result<String, String> {
    let relative: String = path.chars().skip_while(|c| is_path_separator(*c)).collect();

    let full_path = if relative.is_empty() {
        PathBuf::from(base_dir)
    } else {
        let mut base = PathBuf::from(base_dir);
        base.push(&relative);
        base
    };

    let mut components = Vec::new();
    for comp in full_path.components() {
        match comp {
            std::path::Component::ParentDir => {
                if components.pop().is_none() {
                    return Err("路径遍历攻击: 路径超出共享目录范围".to_string());
                }
            }
            std::path::Component::CurDir => {}
            std::path::Component::Normal(c) => {
                components.push(c.to_string_lossy().to_string());
            }
            std::path::Component::RootDir => {}
            std::path::Component::Prefix(p) => {
                components.push(p.as_os_str().to_string_lossy().to_string());
            }
        }
    }

    let base_path = PathBuf::from(base_dir);
    let mut base_components = Vec::new();
    for comp in base_path.components() {
        if let std::path::Component::Normal(c) = comp {
            base_components.push(c.to_string_lossy().to_string());
        } else if let std::path::Component::Prefix(p) = comp {
            base_components.push(p.as_os_str().to_string_lossy().to_string());
        }
    }

    if components.len() < base_components.len() {
        return Err("路径遍历攻击: 路径超出共享目录范围".to_string());
    }
    #[cfg(target_os = "windows")]
    {
        for (i, bc) in base_components.iter().enumerate() {
            if components[i].to_lowercase() != bc.to_lowercase() {
                return Err("路径遍历攻击: 路径超出共享目录范围".to_string());
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        for (i, bc) in base_components.iter().enumerate() {
            if components[i] != *bc {
                return Err("路径遍历攻击: 路径超出共享目录范围".to_string());
            }
        }
    }

    let mut result = PathBuf::new();
    #[cfg(target_os = "windows")]
    {
        if let Some(first) = components.first() {
            if first.len() >= 2 && first.as_bytes()[1] == b':' {
                result.push(first);
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        result.push("/");
    }

    let start = if cfg!(target_os = "windows") && !result.as_os_str().is_empty() {
        1
    } else {
        0
    };
    for comp in components.iter().skip(start) {
        result.push(comp);
    }

    let result_str = result.to_string_lossy().to_string();

    if std::fs::symlink_metadata(&result).is_ok() {
        let provider = default_provider();
        match provider.detect_reparse_point(&result) {
            Ok(true) => {
                return Err(format!("安全拦截: 检测到重解析点（符号链接/junction），拒绝访问: {}", result_str));
            }
            Ok(false) => {}
            Err(_) => {}
        }
    }

    Ok(result_str)
}

pub fn sanitize_path_with_roots(path: &str, share_roots: &[String]) -> Result<String, String> {
    if share_roots.is_empty() {
        return Err("共享根目录白名单为空".to_string());
    }
    for root in share_roots {
        if let Ok(result) = sanitize_path(path, root) {
            return Ok(result);
        }
    }
    Err("路径不在任何共享根目录范围内".to_string())
}

pub fn validate_filename(name: &str) -> Result<(), String> {
    let invalid_chars = ['/', '\\', ':', '*', '?', '"', '<', '>', '|'];
    for ch in invalid_chars {
        if name.contains(ch) {
            return Err(format!("文件名包含非法字符: {}", ch));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_traversal_attack() {
        let result = sanitize_path("/share/../other", "/share");
        assert!(result.is_err());
    }

    #[test]
    fn test_normal_path() {
        let tmp = std::env::temp_dir();
        let base = tmp.to_string_lossy().to_string();
        std::fs::create_dir_all(&base).ok();
        let result = sanitize_path("", &base);
        assert!(result.is_ok());
    }

    #[test]
    fn test_empty_path_returns_base() {
        let result = sanitize_path("/", "/tmp");
        assert!(result.is_ok());
    }

    #[test]
    fn test_dotdot_within_base() {
        let result = sanitize_path("/a/../b", "/tmp");
        assert!(result.is_ok());
    }

    #[test]
    fn test_filename_validation() {
        assert!(validate_filename("normal.txt").is_ok());
        assert!(validate_filename("a/b").is_err());
        assert!(validate_filename("a:b").is_err());
    }
}
