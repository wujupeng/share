use actix_web::{web, HttpResponse, http::StatusCode};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use rusqlite::Connection;
use crate::config::AppConfig;
use crate::error::ApiResponse;
use crate::repositories::user_repo::UserRepo;

#[derive(Deserialize)]
pub struct SetupAdminRequest {
    #[serde(default)]
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct SetupStatus {
    pub admin_initialized: bool,
}

fn is_initialized(conn: &Connection) -> bool {
    let v: i32 = conn
        .query_row(
            "SELECT admin_initialized FROM setup_state WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    v != 0
}

pub async fn setup_status(
    db: web::Data<Arc<Mutex<Connection>>>,
) -> HttpResponse {
    let conn = db.lock().unwrap();
    let initialized = is_initialized(&conn);
    ApiResponse::success(SetupStatus {
        admin_initialized: initialized,
    })
}

pub async fn setup_admin(
    req: web::Json<SetupAdminRequest>,
    db: web::Data<Arc<Mutex<Connection>>>,
    config: web::Data<AppConfig>,
    flag: web::Data<Arc<AtomicBool>>,
) -> HttpResponse {
    {
        let conn = db.lock().unwrap();
        if is_initialized(&conn) {
            return ApiResponse::<()>::error(
                StatusCode::CONFLICT,
                4090,
                "SETUP_ALREADY_DONE",
            );
        }
    }

    if req.password.len() < 8 {
        return ApiResponse::<()>::error(
            StatusCode::BAD_REQUEST,
            4001,
            "WEAK_PASSWORD: 密码长度至少8位",
        );
    }
    let has_upper = req.password.chars().any(|c| c.is_uppercase());
    let has_lower = req.password.chars().any(|c| c.is_lowercase());
    let has_digit = req.password.chars().any(|c| c.is_ascii_digit());
    if !(has_upper && has_lower && has_digit) {
        return ApiResponse::<()>::error(
            StatusCode::BAD_REQUEST,
            4001,
            "WEAK_PASSWORD: 需包含大小写字母与数字",
        );
    }

    let hash = match bcrypt::hash(&req.password, config.auth.bcrypt_cost) {
        Ok(h) => h,
        Err(e) => {
            return ApiResponse::<()>::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                5000,
                &format!("密码哈希失败: {}", e),
            )
        }
    };

    let username = if req.username.is_empty() {
        "admin".to_string()
    } else {
        req.username.clone()
    };

    {
        let conn = db.lock().unwrap();
        if let Err(e) = UserRepo::create(&conn, &username, &hash, "admin") {
            if e.to_string().contains("UNIQUE") {
                if let Err(e) = UserRepo::update_password(&conn, &username, &hash) {
                    return ApiResponse::<()>::error(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        5001,
                        &format!("更新管理员密码失败: {}", e),
                    );
                }
            } else {
                return ApiResponse::<()>::error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    5002,
                    &format!("创建管理员失败: {}", e),
                );
            }
        }
        let _ = conn.execute(
            "UPDATE setup_state SET admin_initialized = 1, initialized_at = datetime('now', 'localtime') WHERE id = 1",
            [],
        );
    }

    flag.store(true, Ordering::Relaxed);
    ApiResponse::<()>::success_message("管理员设置成功")
}