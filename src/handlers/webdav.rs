use std::path::Path;
use actix_web::{web, HttpRequest, HttpResponse};
use chrono::{DateTime, Utc};

use crate::error::AppError;
use crate::services::auth_service::AuthService;
use crate::services::file_service::FileService;
use crate::utils::path_sanitizer::sanitize_path;

fn extract_basic_auth(req: &HttpRequest) -> Option<(String, String)> {
    let header = req.headers().get("Authorization")?.to_str().ok()?;
    if !header.starts_with("Basic ") { return None; }
    let encoded = &header[6..];
    let decoded = base64_decode(encoded)?;
    let mut parts = decoded.splitn(2, ':');
    let user = parts.next()?.to_string();
    let pass = parts.next()?.to_string();
    Some((user, pass))
}

fn base64_decode(input: &str) -> Option<String> {
    let chars = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut bits = 0u32;
    let mut count = 0u32;
    let mut result = Vec::new();
    for c in input.chars() {
        if c == '=' { break; }
        let val = chars.find(c)? as u32;
        bits = (bits << 6) | val;
        count += 6;
        if count >= 8 {
            count -= 8;
            result.push(((bits >> count) & 0xFF) as u8);
        }
    }
    String::from_utf8(result).ok()
}

fn dav_path_to_relative(req_path: &str) -> String {
    let stripped = req_path.strip_prefix("/dav").unwrap_or(req_path);
    if stripped.is_empty() || stripped == "/" {
        "/".to_string()
    } else {
        let decoded = urlencoding::decode(stripped)
            .map(|s| s.to_string())
            .unwrap_or_else(|_| stripped.to_string());
        if decoded.starts_with('/') { decoded } else { format!("/{}", decoded) }
    }
}

fn relative_to_dav_href(rel_path: &str) -> String {
    let encoded = rel_path
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|s| urlencoding::encode(s).to_string())
        .collect::<Vec<_>>()
        .join("/");
    format!("/dav/{}", encoded)
}

fn format_rfc1123(dt: &DateTime<Utc>) -> String {
    dt.format("%a, %d %b %Y %H:%M:%S GMT").to_string()
}

fn format_iso8601(dt: &DateTime<Utc>) -> String {
    dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

fn verify_webdav(
    req: &HttpRequest,
    auth_service: &AuthService,
) -> Result<String, AppError> {
    let (user, pass) = extract_basic_auth(req)
        .ok_or_else(|| AppError::Unauthorized("需要 Basic 认证".to_string()))?;
    auth_service.verify_credentials(&user, &pass)
}

pub async fn options(
    req: HttpRequest,
    auth_service: web::Data<AuthService>,
) -> Result<HttpResponse, AppError> {
    let _role = verify_webdav(&req, &auth_service)?;
    Ok(HttpResponse::Ok()
        .insert_header(("DAV", "1, 2"))
        .insert_header(("MS-Author-Via", "DAV"))
        .insert_header(("Allow", "OPTIONS, PROPFIND, GET, PUT, MKCOL, DELETE, MOVE, COPY, HEAD"))
        .finish())
}

pub async fn propfind(
    req: HttpRequest,
    auth_service: web::Data<AuthService>,
    file_service: web::Data<FileService>,
    payload: web::Payload,
) -> Result<HttpResponse, AppError> {
    let _role = verify_webdav(&req, &auth_service)?;
    let rel_path = dav_path_to_relative(req.path());
    let depth = req.headers().get("Depth")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("1");

    let safe_path = sanitize_path(&rel_path, &file_service.base_dir)
        .map_err(|e| AppError::PathTraversal(e))?;
    let full_path = Path::new(&safe_path);

    if !full_path.exists() {
        return Err(AppError::NotFound("资源不存在".to_string()));
    }

    let _ = payload;

    let mut responses = Vec::new();
    let metadata = std::fs::metadata(full_path)
        .map_err(|e| AppError::Internal(format!("读取元数据失败: {}", e)))?;
    let modified = metadata.modified()
        .map(|t| DateTime::<Utc>::from(t))
        .unwrap_or_else(|_| Utc::now());

    responses.push(build_response_entry(&rel_path, &metadata, &modified));

    if metadata.is_dir() && depth != "0" {
        let entries = std::fs::read_dir(full_path)
            .map_err(|e| AppError::Internal(format!("读取目录失败: {}", e)))?;
        for entry in entries {
            let entry = entry.map_err(|e| AppError::Internal(format!("读取条目失败: {}", e)))?;
            let file_name = entry.file_name().to_string_lossy().to_string();
            if file_name.starts_with('.') { continue; }
            let entry_meta = entry.metadata()
                .map_err(|e| AppError::Internal(format!("读取元数据失败: {}", e)))?;
            let entry_modified = entry_meta.modified()
                .map(|t| DateTime::<Utc>::from(t))
                .unwrap_or_else(|_| Utc::now());
            let child_rel = if rel_path.ends_with('/') {
                format!("{}{}", rel_path, file_name)
            } else {
                format!("{}/{}", rel_path, file_name)
            };
            responses.push(build_response_entry(&child_rel, &entry_meta, &entry_modified));
        }
    }

    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
    xml.push_str("<D:multistatus xmlns:D=\"DAV:\">\n");
    for r in &responses { xml.push_str(r); }
    xml.push_str("</D:multistatus>");

    Ok(HttpResponse::MultiStatus()
        .content_type("application/xml; charset=utf-8")
        .body(xml))
}

fn build_response_entry(rel_path: &str, metadata: &std::fs::Metadata, modified: &DateTime<Utc>) -> String {
    let href = relative_to_dav_href(rel_path);
    let name = rel_path.rsplit('/').next().unwrap_or("").to_string();
    let display_name = if name.is_empty() { "root" } else { &name };
    let is_dir = metadata.is_dir();
    let size = metadata.len();
    let last_mod = format_rfc1123(modified);
    let created = format_iso8601(modified);

    let resource_type = if is_dir {
        "<D:resourcetype><D:collection/></D:resourcetype>".to_string()
    } else {
        "<D:resourcetype/>".to_string()
    };

    let content_length = if is_dir {
        String::new()
    } else {
        format!("<D:getcontentlength>{}</D:getcontentlength>", size)
    };

    let content_type = if is_dir {
        String::new()
    } else {
        let mime = mime_guess::from_path(rel_path).first_or_octet_stream().to_string();
        format!("<D:getcontenttype>{}</D:getcontenttype>", mime)
    };

    format!(
        "  <D:response>\n    <D:href>{}</D:href>\n    <D:propstat>\n      <D:prop>\n        <D:displayname>{}</D:displayname>\n        {}\n        {}\n        {}\n        <D:getlastmodified>{}</D:getlastmodified>\n        <D:creationdate>{}</D:creationdate>\n      </D:prop>\n      <D:status>HTTP/1.1 200 OK</D:status>\n    </D:propstat>\n  </D:response>\n",
        href, display_name, resource_type, content_length, content_type, last_mod, created
    )
}

pub async fn get_file(
    req: HttpRequest,
    auth_service: web::Data<AuthService>,
    file_service: web::Data<FileService>,
) -> Result<HttpResponse, AppError> {
    let _role = verify_webdav(&req, &auth_service)?;
    let rel_path = dav_path_to_relative(req.path());
    let safe_path = sanitize_path(&rel_path, &file_service.base_dir)
        .map_err(|e| AppError::PathTraversal(e))?;

    let path = Path::new(&safe_path);
    if !path.is_file() {
        return Err(AppError::NotFound("文件不存在".to_string()));
    }

    let file = actix_files::NamedFile::open(path)
        .map_err(|e| AppError::Internal(format!("打开文件失败: {}", e)))?;
    Ok(file.into_response(&req))
}

pub async fn head_file(
    req: HttpRequest,
    auth_service: web::Data<AuthService>,
    file_service: web::Data<FileService>,
) -> Result<HttpResponse, AppError> {
    let _role = verify_webdav(&req, &auth_service)?;
    let rel_path = dav_path_to_relative(req.path());
    let safe_path = sanitize_path(&rel_path, &file_service.base_dir)
        .map_err(|e| AppError::PathTraversal(e))?;

    let path = Path::new(&safe_path);
    if !path.exists() {
        return Err(AppError::NotFound("资源不存在".to_string()));
    }
    let metadata = std::fs::metadata(path)
        .map_err(|e| AppError::Internal(format!("读取元数据失败: {}", e)))?;
    let mime = mime_guess::from_path(path).first_or_octet_stream().to_string();
    Ok(HttpResponse::Ok()
        .content_type(mime)
        .insert_header(("Content-Length", metadata.len().to_string()))
        .finish())
}

pub async fn put_file(
    req: HttpRequest,
    auth_service: web::Data<AuthService>,
    file_service: web::Data<FileService>,
    payload: web::Payload,
) -> Result<HttpResponse, AppError> {
    let _role = verify_webdav(&req, &auth_service)?;
    let rel_path = dav_path_to_relative(req.path());
    let safe_path = sanitize_path(&rel_path, &file_service.base_dir)
        .map_err(|e| AppError::PathTraversal(e))?;

    let path = Path::new(&safe_path);
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)
                .map_err(|e| AppError::Internal(format!("创建目录失败: {}", e)))?;
        }
    }

    use futures::StreamExt;
    let mut bytes = web::BytesMut::new();
    let mut stream = payload;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| AppError::Internal(format!("读取数据失败: {}", e)))?;
        bytes.extend_from_slice(&chunk);
    }

    std::fs::write(&safe_path, &bytes)
        .map_err(|e| AppError::Internal(format!("写入文件失败: {}", e)))?;

    let status = if req.method() == actix_web::http::Method::PUT && path.exists() {
        actix_web::http::StatusCode::NO_CONTENT
    } else {
        actix_web::http::StatusCode::CREATED
    };
    Ok(HttpResponse::build(status).finish())
}

pub async fn mkcol(
    req: HttpRequest,
    auth_service: web::Data<AuthService>,
    file_service: web::Data<FileService>,
) -> Result<HttpResponse, AppError> {
    let _role = verify_webdav(&req, &auth_service)?;
    let rel_path = dav_path_to_relative(req.path());
    let safe_path = sanitize_path(&rel_path, &file_service.base_dir)
        .map_err(|e| AppError::PathTraversal(e))?;

    let path = Path::new(&safe_path);
    if path.exists() {
        return Err(AppError::Conflict("资源已存在".to_string()));
    }
    std::fs::create_dir_all(path)
        .map_err(|e| AppError::Internal(format!("创建目录失败: {}", e)))?;
    Ok(HttpResponse::Created().finish())
}

pub async fn delete(
    req: HttpRequest,
    auth_service: web::Data<AuthService>,
    file_service: web::Data<FileService>,
) -> Result<HttpResponse, AppError> {
    let _role = verify_webdav(&req, &auth_service)?;
    let rel_path = dav_path_to_relative(req.path());
    let safe_path = sanitize_path(&rel_path, &file_service.base_dir)
        .map_err(|e| AppError::PathTraversal(e))?;

    let path = Path::new(&safe_path);
    if !path.exists() {
        return Err(AppError::NotFound("资源不存在".to_string()));
    }
    if path.is_dir() {
        std::fs::remove_dir_all(path)
            .map_err(|e| AppError::Internal(format!("删除目录失败: {}", e)))?;
    } else {
        std::fs::remove_file(path)
            .map_err(|e| AppError::Internal(format!("删除文件失败: {}", e)))?;
    }
    Ok(HttpResponse::NoContent().finish())
}

pub async fn move_resource(
    req: HttpRequest,
    auth_service: web::Data<AuthService>,
    file_service: web::Data<FileService>,
) -> Result<HttpResponse, AppError> {
    let _role = verify_webdav(&req, &auth_service)?;
    let src_rel = dav_path_to_relative(req.path());
    let dest_header = req.headers().get("Destination")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::BadRequest("缺少 Destination 头".to_string()))?;

    let dest_path = if let Some(idx) = dest_header.find("/dav") {
        &dest_header[idx..]
    } else {
        dest_header
    };
    let dest_rel = dav_path_to_relative(dest_path);

    let src_safe = sanitize_path(&src_rel, &file_service.base_dir)
        .map_err(|e| AppError::PathTraversal(e))?;
    let dest_safe = sanitize_path(&dest_rel, &file_service.base_dir)
        .map_err(|e| AppError::PathTraversal(e))?;

    let src = Path::new(&src_safe);
    let dest = Path::new(&dest_safe);
    if !src.exists() {
        return Err(AppError::NotFound("源资源不存在".to_string()));
    }
    if dest.exists() {
        let overwrite = req.headers().get("Overwrite")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("F");
        if overwrite == "F" {
            return Err(AppError::Conflict("目标已存在".to_string()));
        }
        if dest.is_dir() {
            std::fs::remove_dir_all(dest)
                .map_err(|e| AppError::Internal(format!("删除目标目录失败: {}", e)))?;
        } else {
            std::fs::remove_file(dest)
                .map_err(|e| AppError::Internal(format!("删除目标文件失败: {}", e)))?;
        }
    }
    if let Some(parent) = dest.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)
                .map_err(|e| AppError::Internal(format!("创建目录失败: {}", e)))?;
        }
    }
    std::fs::rename(src, dest)
        .map_err(|e| AppError::Internal(format!("移动失败: {}", e)))?;
    Ok(HttpResponse::Created().finish())
}

pub async fn copy_resource(
    req: HttpRequest,
    auth_service: web::Data<AuthService>,
    file_service: web::Data<FileService>,
) -> Result<HttpResponse, AppError> {
    let _role = verify_webdav(&req, &auth_service)?;
    let src_rel = dav_path_to_relative(req.path());
    let dest_header = req.headers().get("Destination")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::BadRequest("缺少 Destination 头".to_string()))?;

    let dest_path = if let Some(idx) = dest_header.find("/dav") {
        &dest_header[idx..]
    } else {
        dest_header
    };
    let dest_rel = dav_path_to_relative(dest_path);

    let src_safe = sanitize_path(&src_rel, &file_service.base_dir)
        .map_err(|e| AppError::PathTraversal(e))?;
    let dest_safe = sanitize_path(&dest_rel, &file_service.base_dir)
        .map_err(|e| AppError::PathTraversal(e))?;

    let src = Path::new(&src_safe);
    let dest = Path::new(&dest_safe);
    if !src.exists() {
        return Err(AppError::NotFound("源资源不存在".to_string()));
    }
    if dest.exists() {
        let overwrite = req.headers().get("Overwrite")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("F");
        if overwrite == "F" {
            return Err(AppError::Conflict("目标已存在".to_string()));
        }
        if dest.is_dir() {
            std::fs::remove_dir_all(dest)
                .map_err(|e| AppError::Internal(format!("删除目标目录失败: {}", e)))?;
        } else {
            std::fs::remove_file(dest)
                .map_err(|e| AppError::Internal(format!("删除目标文件失败: {}", e)))?;
        }
    }
    if let Some(parent) = dest.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)
                .map_err(|e| AppError::Internal(format!("创建目录失败: {}", e)))?;
        }
    }
    if src.is_dir() {
        copy_dir_recursive(src, dest)
            .map_err(|e| AppError::Internal(format!("复制目录失败: {}", e)))?;
    } else {
        std::fs::copy(src, dest)
            .map_err(|e| AppError::Internal(format!("复制文件失败: {}", e)))?;
    }
    Ok(HttpResponse::Created().finish())
}

fn copy_dir_recursive(src: &Path, dest: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dest)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dest_path = dest.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dest_path)?;
        } else {
            std::fs::copy(&src_path, &dest_path)?;
        }
    }
    Ok(())
}