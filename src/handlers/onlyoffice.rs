use actix_web::{web, HttpRequest, HttpResponse};
use serde::Deserialize;
use std::io::Read;
use std::path::Path;
use std::sync::{Mutex, Arc};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use rusqlite::Connection;
use jsonwebtoken::{encode, EncodingKey, Header};
use crate::error::AppError;
use crate::services::file_service::FileService;
use crate::services::auth_service::AuthService;
use crate::utils::path_sanitizer::sanitize_path;
use crate::middleware::auth::extract_token_from_header;
use crate::repositories::permission_repo::PermissionRepo;
use crate::repositories::audit_repo::AuditRepo;

const ONLYOFFICE_JWT_SECRET: &str = "KJ16GbPqfeCkyAVcfC5zptCptsEwIf7E";
const ONLYOFFICE_SERVER: &str = "http://192.168.2.114:8080";
const SHAREWEB_EXTERNAL_URL: &str = "http://192.168.2.142:18888";

#[derive(Deserialize)]
pub struct ConfigQuery {
    pub path: String,
    pub mode: Option<String>,
}

#[derive(Deserialize)]
pub struct CallbackQuery {
    pub path: String,
    pub token: String,
}

#[derive(Deserialize)]
pub struct CallbackRequest {
    pub status: i32,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub users: Vec<String>,
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

fn get_document_type(ext: &str) -> Option<&'static str> {
    match ext {
        "doc" | "docx" | "rtf" | "odt" | "txt" | "wps" => Some("word"),
        "xls" | "xlsx" | "ods" | "csv" | "et" => Some("spreadsheet"),
        "ppt" | "pptx" | "odp" | "dps" => Some("presentation"),
        _ => None,
    }
}

fn sign_jwt(value: &serde_json::Value) -> Result<String, AppError> {
    encode(
        &Header::default(),
        value,
        &EncodingKey::from_secret(ONLYOFFICE_JWT_SECRET.as_bytes()),
    ).map_err(|e| AppError::Internal(format!("JWT 签名失败: {}", e)))
}

pub async fn get_config(
    file_service: web::Data<FileService>,
    auth_service: web::Data<AuthService>,
    db: web::Data<Arc<Mutex<Connection>>>,
    query: web::Query<ConfigQuery>,
    req: HttpRequest,
) -> Result<HttpResponse, AppError> {
    let token = extract_token_from_header(req.headers())?;
    let claims = auth_service.verify_token(&token)?;
    let username = claims.sub;
    let role = claims.role;

    let allowed_paths = get_allowed_paths(&db, &role);
    if !is_path_allowed(&query.path, &allowed_paths) {
        return Err(AppError::Forbidden("无权访问此路径".to_string()));
    }

    let safe_path = sanitize_path(&query.path, &file_service.base_dir)
        .map_err(|e| AppError::PathTraversal(e))?;
    let full_path = Path::new(&safe_path);

    if !full_path.exists() {
        return Err(AppError::NotFound("文件不存在".to_string()));
    }
    if full_path.is_dir() {
        return Err(AppError::BadRequest("不能编辑目录".to_string()));
    }

    let file_name = full_path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("document")
        .to_string();

    let extension = full_path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let document_type = get_document_type(&extension)
        .ok_or_else(|| AppError::BadRequest(format!("不支持编辑 .{} 文件", extension)))?;

    let metadata = tokio::fs::metadata(&full_path).await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let modified = metadata.modified()
        .map(|t| t.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0))
        .unwrap_or(0);

    let mut hasher = DefaultHasher::new();
    query.path.hash(&mut hasher);
    modified.hash(&mut hasher);
    let doc_key = format!("{:016x}", hasher.finish());

    let file_url = format!(
        "{}/api/files/download?path={}&token={}",
        SHAREWEB_EXTERNAL_URL,
        urlencoding::encode(&query.path),
        urlencoding::encode(&token)
    );

    let callback_url = format!(
        "{}/api/files/onlyoffice/callback?path={}&token={}",
        SHAREWEB_EXTERNAL_URL,
        urlencoding::encode(&query.path),
        urlencoding::encode(&token)
    );

    let mode = match query.mode.as_deref() {
        Some("view") => "view",
        _ => if role == "admin" || role == "readwrite" { "edit" } else { "view" },
    };

    let document = serde_json::json!({
        "title": file_name,
        "url": file_url,
        "fileType": extension,
        "key": doc_key,
    });

    let editor_config = serde_json::json!({
        "mode": mode,
        "lang": "zh",
        "callbackUrl": callback_url,
        "user": {
            "id": username,
            "name": username,
        },
        "customized": {
            "forcesave": true,
            "compactHeader": true,
            "toolbarNoTabs": false,
        }
    });

    let doc_token = sign_jwt(&document)?;
    let editor_token = sign_jwt(&editor_config)?;

    let mut document_with_token = document.clone();
    document_with_token["token"] = serde_json::Value::String(doc_token);

    let mut editor_config_with_token = editor_config.clone();
    editor_config_with_token["token"] = serde_json::Value::String(editor_token);

    let config = serde_json::json!({
        "document": document_with_token,
        "documentType": document_type,
        "editorConfig": editor_config_with_token,
        "height": "100%",
        "width": "100%",
    });

    let config_token = sign_jwt(&config)?;

    let mut config_with_token = config.clone();
    config_with_token["token"] = serde_json::Value::String(config_token);

    if let Ok(conn) = db.lock() {
        let source_ip = req.connection_info().peer_addr().unwrap_or("unknown").to_string();
        let _ = AuditRepo::insert(&conn, &username, "onlyoffice_edit", Some(&query.path), None, &source_ip, "success", None);
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "code": 0,
        "message": "success",
        "data": {
            "config": config_with_token,
            "server": ONLYOFFICE_SERVER,
        }
    })))
}

pub async fn callback(
    file_service: web::Data<FileService>,
    auth_service: web::Data<AuthService>,
    db: web::Data<Arc<Mutex<Connection>>>,
    query: web::Query<CallbackQuery>,
    body: web::Json<CallbackRequest>,
    req: HttpRequest,
) -> Result<HttpResponse, AppError> {
    let claims = auth_service.verify_token(&query.token)?;
    let username = claims.sub;
    let role = claims.role;

    let allowed_paths = get_allowed_paths(&db, &role);
    if !is_path_allowed(&query.path, &allowed_paths) {
        return Err(AppError::Forbidden("无权访问此路径".to_string()));
    }

    if (body.status == 2 || body.status == 6) && body.url.is_some() {
        let download_url = body.url.clone().unwrap();
        let path = query.path.clone();
        let base_dir = file_service.base_dir.clone();

        tokio::task::spawn_blocking(move || {
            let resp = ureq::get(&download_url).call()
                .map_err(|e| AppError::Internal(format!("下载编辑后文件失败: {}", e)))?;
            let mut buf = Vec::new();
            resp.into_reader().read_to_end(&mut buf)
                .map_err(|e| AppError::Internal(format!("读取文件内容失败: {}", e)))?;

            let safe_path = sanitize_path(&path, &base_dir)
                .map_err(|e| AppError::PathTraversal(e))?;
            std::fs::write(&safe_path, &buf)
                .map_err(|e| AppError::Internal(format!("保存文件失败: {}", e)))?;
            Ok::<(), AppError>(())
        }).await.map_err(|e| AppError::Internal(e.to_string()))??;

        if let Ok(conn) = db.lock() {
            let source_ip = req.connection_info().peer_addr().unwrap_or("unknown").to_string();
            let _ = AuditRepo::insert(&conn, &username, "onlyoffice_save", Some(&query.path), None, &source_ip, "success", None);
        }
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({"error": 0})))
}