use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::sync::{Mutex, Arc};

use crate::config::AppConfig;
use crate::error::AppError;
use crate::models::user::{UserStatus, UserRole};
use crate::repositories::user_repo::UserRepo;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub role: String,
    pub exp: usize,
    pub iat: usize,
}

pub struct AuthService {
    pub db: Arc<Mutex<Connection>>,
    pub config: AppConfig,
}

impl AuthService {
    pub fn new(db: Arc<Mutex<Connection>>, config: AppConfig) -> Self {
        Self { db, config }
    }

    pub fn login(&self, username: &str, password: &str) -> Result<(String, String, i64), AppError> {
        let conn = self.db.lock().map_err(|e| AppError::Internal(e.to_string()))?;

        let user = UserRepo::find_by_username(&conn, username)
            .map_err(|e| AppError::Internal(e.to_string()))?
            .ok_or_else(|| AppError::Unauthorized("用户名或密码错误".to_string()))?;

        if user.status == UserStatus::Disabled {
            return Err(AppError::Unauthorized("账户已禁用".to_string()));
        }

        if user.status == UserStatus::Locked {
            if let Some(ref locked_until) = user.locked_until {
                if let Ok(lock_time) = chrono::DateTime::parse_from_rfc3339(&format!("{}+00:00", locked_until)) {
                    if Utc::now() < lock_time.with_timezone(&Utc) {
                        return Err(AppError::Unauthorized(format!("账户已锁定，请于 {} 后重试", locked_until)));
                    }
                }
            }
        }

        let password_valid = bcrypt::verify(password, &user.password_hash)
            .map_err(|e| AppError::Internal(e.to_string()))?;

        if !password_valid {
            let new_count = user.login_fail_count + 1;
            if new_count >= self.config.auth.max_login_attempts {
                let lock_until = Utc::now() + chrono::Duration::minutes(self.config.auth.lock_duration_minutes as i64);
                UserRepo::update_login_fail(&conn, username, new_count, Some(&lock_until.to_rfc3339()))
                    .map_err(|e| AppError::Internal(e.to_string()))?;
                return Err(AppError::Unauthorized(format!("登录失败次数过多，账户已锁定{}分钟", self.config.auth.lock_duration_minutes)));
            } else {
                UserRepo::update_login_fail(&conn, username, new_count, None)
                    .map_err(|e| AppError::Internal(e.to_string()))?;
            }
            return Err(AppError::Unauthorized("用户名或密码错误".to_string()));
        }

        UserRepo::update_login_fail(&conn, username, 0, None)
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let now = Utc::now();
        let expiration = now + chrono::Duration::hours(self.config.auth.jwt_expiration_hours);
        let claims = Claims {
            sub: username.to_string(),
            role: user.role.as_str().to_string(),
            exp: expiration.timestamp() as usize,
            iat: now.timestamp() as usize,
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.config.auth.jwt_secret.as_bytes()),
        ).map_err(|e| AppError::Internal(format!("令牌生成失败: {}", e)))?;

        Ok((token, user.role.as_str().to_string(), self.config.auth.jwt_expiration_hours * 3600))
    }

    pub fn verify_credentials(&self, username: &str, password: &str) -> Result<String, AppError> {
        let conn = self.db.lock().map_err(|e| AppError::Internal(e.to_string()))?;
        let user = UserRepo::find_by_username(&conn, username)
            .map_err(|e| AppError::Internal(e.to_string()))?
            .ok_or_else(|| AppError::Unauthorized("用户名或密码错误".to_string()))?;

        if user.status == UserStatus::Disabled {
            return Err(AppError::Unauthorized("账户已禁用".to_string()));
        }

        let valid = bcrypt::verify(password, &user.password_hash)
            .map_err(|e| AppError::Internal(e.to_string()))?;
        if !valid {
            return Err(AppError::Unauthorized("用户名或密码错误".to_string()));
        }

        Ok(user.role.as_str().to_string())
    }

    pub fn verify_token(&self, token: &str) -> Result<Claims, AppError> {
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.config.auth.jwt_secret.as_bytes()),
            &Validation::default(),
        ).map_err(|e| match e.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => AppError::TokenExpired("令牌已过期".to_string()),
            _ => AppError::Unauthorized(format!("无效令牌: {}", e)),
        })?;
        Ok(token_data.claims)
    }

    pub fn change_password(&self, username: &str, old_password: &str, new_password: &str) -> Result<(), AppError> {
        let conn = self.db.lock().map_err(|e| AppError::Internal(e.to_string()))?;

        let user = UserRepo::find_by_username(&conn, username)
            .map_err(|e| AppError::Internal(e.to_string()))?
            .ok_or_else(|| AppError::Unauthorized("用户不存在".to_string()))?;

        let valid = bcrypt::verify(old_password, &user.password_hash)
            .map_err(|e| AppError::Internal(e.to_string()))?;
        if !valid {
            return Err(AppError::BadRequest("旧密码错误".to_string()));
        }

        let new_hash = bcrypt::hash(new_password, self.config.auth.bcrypt_cost)
            .map_err(|e| AppError::Internal(format!("密码哈希失败: {}", e)))?;

        UserRepo::update_password(&conn, username, &new_hash)
            .map_err(|e| AppError::Internal(e.to_string()))?;

        Ok(())
    }
}
