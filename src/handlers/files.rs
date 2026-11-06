use actix_web::{web, HttpRequest, HttpResponse};
use serde::Deserialize;
use std::sync::{Mutex, Arc};
use rusqlite::Connection;
use crate::error::AppError;
use crate::services::file_service::FileService;
use crate::middleware::auth::extract_token_from_header;
use crate::services::auth_service::AuthService;
use crate::repositories::permission_repo::PermissionRepo;
use crate::repositories::audit_repo::AuditRepo;
use crate::utils::path_sanitizer::sanitize_path;

#[derive(Deserialize)]
pub struct ListQuery {
    pub path: Option<String>,
    pub show_hidden: Option<bool>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

#[derive(Deserialize)]
pub struct TreeQuery {
    pub path: Option<String>,
}

#[derive(Deserialize)]
pub struct SearchQuery {
    pub keyword: String,
    pub path: Option<String>,
    pub max_depth: Option<u32>,
}

#[derive(Deserialize)]
pub struct MkdirRequest {
    pub parent_path: String,
    pub name: String,
}

#[derive(Deserialize)]
pub struct RenameRequest {
    pub path: String,
    pub new_name: String,
}

#[derive(Deserialize)]
pub struct DeleteRequest {
    pub path: String,
}

#[derive(Deserialize)]
pub struct MoveRequest {
    pub source_path: String,
    pub target_dir: String,
}

#[derive(Deserialize)]
pub struct CopyRequest {
    pub source_path: String,
    pub target_dir: String,
}

fn get_user_role(req: &HttpRequest, auth_service: &AuthService) -> Result<String, AppError> {
    let token = extract_token_from_header(req.headers())?;
    let claims = auth_service.verify_token(&token)?;
    Ok(claims.role)
}

fn get_user_username(req: &HttpRequest, auth_service: &AuthService) -> Result<String, AppError> {
    let token = extract_token_from_header(req.headers())?;
    let claims = auth_service.verify_token(&token)?;
    Ok(claims.sub)
}

fn get_allowed_paths(db: &Arc<Mutex<Connection>>, role: &str) -> Vec<String> {
    if role == "admin" {
        return vec!["/".to_string()];
    }
    let conn = db.lock().unwrap();
    let all_rules = PermissionRepo::list(&conn, None).unwrap_or_default();
    all_rules.iter()
        .filter(|r| r.role == role)
        .map(|r| r.path.clone())
        .collect()
}

fn is_path_allowed(requested_path: &str, allowed_paths: &[String]) -> bool {
    if allowed_paths.contains(&"/".to_string()) {
        return true;
    }
    for allowed in allowed_paths {
        let norm_allowed = allowed.trim_end_matches('/');
        let norm_requested = requested_path.trim_end_matches('/');
        if norm_requested == norm_allowed
            || norm_requested.starts_with(&format!("{}/", norm_allowed))
            || norm_allowed.starts_with(&format!("{}/", norm_requested))
        {
            return true;
        }
    }
    false
}

fn require_write_access(role: &str) -> Result<(), AppError> {
    if role != "admin" && role != "readwrite" {
        return Err(AppError::Forbidden("需要写入权限".to_string()));
    }
    Ok(())
}

pub async fn list_files(
    file_service: web::Data<FileService>,
    auth_service: web::Data<AuthService>,
    db: web::Data<Arc<Mutex<Connection>>>,
    req: HttpRequest,
    query: web::Query<ListQuery>,
) -> Result<HttpResponse, AppError> {
    let path = query.path.as_deref().unwrap_or("/");
    let show_hidden = query.show_hidden.unwrap_or(false);
    let sort_by = query.sort_by.as_deref().unwrap_or("name");
    let sort_order = query.sort_order.as_deref().unwrap_or("asc");
    let page = query.page.unwrap_or(1);
    let page_size = query.page_size.unwrap_or(50);

    let role = get_user_role(&req, &auth_service)?;
    let allowed_paths = get_allowed_paths(&db, &role);

    if !is_path_allowed(path, &allowed_paths) {
        return Err(AppError::Forbidden("无权访问此路径".to_string()));
    }

    let all_files = file_service.list_directory(path, show_hidden, sort_by, sort_order).await?;

    let filtered: Vec<_> = all_files.into_iter()
        .filter(|f| is_path_allowed(&f.path, &allowed_paths))
        .collect();

    let total = filtered.len();
    let start = ((page - 1) * page_size) as usize;
    let end = std::cmp::min(start + page_size as usize, total);
    let items: Vec<_> = if start < total {
        filtered[start..end].to_vec()
    } else {
        Vec::new()
    };

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "code": 0,
        "message": "success",
        "data": {
            "items": items,
            "total": total,
            "page": page,
            "page_size": page_size
        }
    })))
}

pub async fn get_tree(
    file_service: web::Data<FileService>,
    auth_service: web::Data<AuthService>,
    db: web::Data<Arc<Mutex<Connection>>>,
    req: HttpRequest,
    query: web::Query<TreeQuery>,
) -> Result<HttpResponse, AppError> {
    let path = query.path.as_deref().unwrap_or("/");

    let role = get_user_role(&req, &auth_service)?;
    let allowed_paths = get_allowed_paths(&db, &role);

    if !is_path_allowed(path, &allowed_paths) {
        return Err(AppError::Forbidden("无权访问此路径".to_string()));
    }

    let tree = file_service.get_tree(path).await?;

    let filtered: Vec<_> = tree.into_iter()
        .filter(|n| is_path_allowed(&n.path, &allowed_paths))
        .collect();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "code": 0,
        "message": "success",
        "data": filtered
    })))
}

pub async fn search_files(
    file_service: web::Data<FileService>,
    auth_service: web::Data<AuthService>,
    db: web::Data<Arc<Mutex<Connection>>>,
    req: HttpRequest,
    query: web::Query<SearchQuery>,
) -> Result<HttpResponse, AppError> {
    let path = query.path.as_deref().unwrap_or("/");
    let max_depth = query.max_depth.unwrap_or(10);

    let role = get_user_role(&req, &auth_service)?;
    let allowed_paths = get_allowed_paths(&db, &role);

    if !is_path_allowed(path, &allowed_paths) {
        return Err(AppError::Forbidden("无权访问此路径".to_string()));
    }

    let results = file_service.search(&query.keyword, path, max_depth).await?;

    let filtered: Vec<_> = results.into_iter()
        .filter(|f| is_path_allowed(&f.path, &allowed_paths))
        .collect();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "code": 0,
        "message": "success",
        "data": filtered
    })))
}

pub async fn mkdir(
    file_service: web::Data<FileService>,
    auth_service: web::Data<AuthService>,
    db: web::Data<Arc<Mutex<Connection>>>,
    req: HttpRequest,
    body: web::Json<MkdirRequest>,
) -> Result<HttpResponse, AppError> {
    let role = get_user_role(&req, &auth_service)?;
    let username = get_user_username(&req, &auth_service)?;
    let source_ip = req.connection_info().peer_addr().unwrap_or("unknown").to_string();
    let allowed_paths = get_allowed_paths(&db, &role);
    require_write_access(&role)?;
    if !is_path_allowed(&body.parent_path, &allowed_paths) {
        return Err(AppError::Forbidden("无权在此路径操作".to_string()));
    }
    file_service.mkdir(&body.parent_path, &body.name).await?;
    if let Ok(conn) = db.lock() {
        let _ = AuditRepo::insert(&conn, &username, "mkdir", Some(&body.parent_path), Some(&body.name), &source_ip, "success", None);
    }
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "code": 0,
        "message": "文件夹创建成功"
    })))
}

pub async fn rename(
    file_service: web::Data<FileService>,
    auth_service: web::Data<AuthService>,
    db: web::Data<Arc<Mutex<Connection>>>,
    req: HttpRequest,
    body: web::Json<RenameRequest>,
) -> Result<HttpResponse, AppError> {
    let role = get_user_role(&req, &auth_service)?;
    let username = get_user_username(&req, &auth_service)?;
    let source_ip = req.connection_info().peer_addr().unwrap_or("unknown").to_string();
    let allowed_paths = get_allowed_paths(&db, &role);
    require_write_access(&role)?;
    if !is_path_allowed(&body.path, &allowed_paths) {
        return Err(AppError::Forbidden("无权在此路径操作".to_string()));
    }
    file_service.rename(&body.path, &body.new_name).await?;
    if let Ok(conn) = db.lock() {
        let _ = AuditRepo::insert(&conn, &username, "rename", Some(&body.path), Some(&body.new_name), &source_ip, "success", None);
    }
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "code": 0,
        "message": "重命名成功"
    })))
}

pub async fn delete(
    file_service: web::Data<FileService>,
    auth_service: web::Data<AuthService>,
    db: web::Data<Arc<Mutex<Connection>>>,
    req: HttpRequest,
    body: web::Json<DeleteRequest>,
) -> Result<HttpResponse, AppError> {
    let role = get_user_role(&req, &auth_service)?;
    let username = get_user_username(&req, &auth_service)?;
    let source_ip = req.connection_info().peer_addr().unwrap_or("unknown").to_string();
    let allowed_paths = get_allowed_paths(&db, &role);
    require_write_access(&role)?;
    if !is_path_allowed(&body.path, &allowed_paths) {
        return Err(AppError::Forbidden("无权在此路径操作".to_string()));
    }
    file_service.delete(&body.path).await?;
    if let Ok(conn) = db.lock() {
        let _ = AuditRepo::insert(&conn, &username, "delete", Some(&body.path), None, &source_ip, "success", None);
    }
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "code": 0,
        "message": "删除成功"
    })))
}

pub async fn move_file(
    file_service: web::Data<FileService>,
    auth_service: web::Data<AuthService>,
    db: web::Data<Arc<Mutex<Connection>>>,
    req: HttpRequest,
    body: web::Json<MoveRequest>,
) -> Result<HttpResponse, AppError> {
    let role = get_user_role(&req, &auth_service)?;
    let username = get_user_username(&req, &auth_service)?;
    let source_ip = req.connection_info().peer_addr().unwrap_or("unknown").to_string();
    let allowed_paths = get_allowed_paths(&db, &role);
    require_write_access(&role)?;
    if !is_path_allowed(&body.source_path, &allowed_paths) || !is_path_allowed(&body.target_dir, &allowed_paths) {
        return Err(AppError::Forbidden("无权在此路径操作".to_string()));
    }
    file_service.move_file(&body.source_path, &body.target_dir).await?;
    if let Ok(conn) = db.lock() {
        let _ = AuditRepo::insert(&conn, &username, "move", Some(&body.source_path), Some(&body.target_dir), &source_ip, "success", None);
    }
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "code": 0,
        "message": "移动成功"
    })))
}

pub async fn copy_file(
    file_service: web::Data<FileService>,
    auth_service: web::Data<AuthService>,
    db: web::Data<Arc<Mutex<Connection>>>,
    req: HttpRequest,
    body: web::Json<CopyRequest>,
) -> Result<HttpResponse, AppError> {
    let role = get_user_role(&req, &auth_service)?;
    let username = get_user_username(&req, &auth_service)?;
    let source_ip = req.connection_info().peer_addr().unwrap_or("unknown").to_string();
    let allowed_paths = get_allowed_paths(&db, &role);
    require_write_access(&role)?;
    if !is_path_allowed(&body.source_path, &allowed_paths) || !is_path_allowed(&body.target_dir, &allowed_paths) {
        return Err(AppError::Forbidden("无权在此路径操作".to_string()));
    }
    file_service.copy_file(&body.source_path, &body.target_dir).await?;
    if let Ok(conn) = db.lock() {
        let _ = AuditRepo::insert(&conn, &username, "copy", Some(&body.source_path), Some(&body.target_dir), &source_ip, "success", None);
    }
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "code": 0,
        "message": "复制成功"
    })))
}
#[derive(Deserialize)]
pub struct PreviewQuery {
    pub path: Option<String>,
}

pub async fn preview_archive(
    file_service: web::Data<FileService>,
    auth_service: web::Data<AuthService>,
    db: web::Data<Arc<Mutex<Connection>>>,
    req: HttpRequest,
    query: web::Query<PreviewQuery>,
) -> Result<HttpResponse, AppError> {
    let path = query.path.as_deref().unwrap_or("/");
    let role = get_user_role(&req, &auth_service)?;
    let allowed_paths = get_allowed_paths(&db, &role);
    if !is_path_allowed(path, &allowed_paths) {
        return Err(AppError::Forbidden("无权访问此路径".to_string()));
    }

    let safe_path = sanitize_path(path, &file_service.base_dir)
        .map_err(|e| AppError::PathTraversal(e))?;

    let file = std::fs::File::open(&safe_path)
        .map_err(|e| AppError::Internal(format!("打开文件失败: {}", e)))?;

    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| AppError::Internal(format!("读取压缩文件失败: {}", e)))?;

    let mut entries = Vec::new();
    for i in 0..archive.len() {
        let entry = archive.by_index(i)
            .map_err(|e| AppError::Internal(format!("读取条目失败: {}", e)))?;
        entries.push(serde_json::json!({
            "name": entry.name().to_string(),
            "size": entry.size(),
            "compressed_size": entry.compressed_size(),
            "is_dir": entry.is_dir(),
        }));
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "code": 0,
        "message": "success",
        "data": {
            "entries": entries,
            "total": entries.len()
        }
    })))
}

pub async fn preview_apk(
    file_service: web::Data<FileService>,
    auth_service: web::Data<AuthService>,
    db: web::Data<Arc<Mutex<Connection>>>,
    req: HttpRequest,
    query: web::Query<PreviewQuery>,
) -> Result<HttpResponse, AppError> {
    let path = query.path.as_deref().unwrap_or("/");
    let role = get_user_role(&req, &auth_service)?;
    let allowed_paths = get_allowed_paths(&db, &role);
    if !is_path_allowed(path, &allowed_paths) {
        return Err(AppError::Forbidden("无权访问此路径".to_string()));
    }

    let safe_path = sanitize_path(path, &file_service.base_dir)
        .map_err(|e| AppError::PathTraversal(e))?;

    let file = std::fs::File::open(&safe_path)
        .map_err(|e| AppError::Internal(format!("打开文件失败: {}", e)))?;

    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| AppError::Internal(format!("读取APK失败: {}", e)))?;

    let mut entries = Vec::new();
    let mut total_size = 0u64;
    for i in 0..archive.len() {
        let entry = archive.by_index(i)
            .map_err(|e| AppError::Internal(format!("读取条目失败: {}", e)))?;
        let name = entry.name().to_string();
        total_size += entry.size();
        if name.contains("AndroidManifest.xml") || name.ends_with(".dex") || name.starts_with("res/") || name.starts_with("assets/") {
            entries.push(serde_json::json!({
                "name": name,
                "size": entry.size(),
                "is_dir": entry.is_dir(),
            }));
        }
    }

    let file_size = std::fs::metadata(&safe_path)
        .map(|m| m.len())
        .unwrap_or(0);

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "code": 0,
        "message": "success",
        "data": {
            "file_size": file_size,
            "uncompressed_size": total_size,
            "entry_count": archive.len(),
            "key_entries": entries
        }
    })))
}