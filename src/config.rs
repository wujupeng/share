use serde::Deserialize;
use std::env;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub storage: StorageConfig,
    pub database: DatabaseConfig,
    pub auth: AuthConfig,
    pub log: LogConfig,
    #[serde(default)]
    pub platform: PlatformConfig,
    #[serde(default)]
    pub security: SecurityConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub workers: usize,
    #[serde(default)]
    pub static_dir: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StorageConfig {
    pub base_dir: String,
    pub max_file_size: i64,
    pub chunk_size: i64,
    pub tmp_dir: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthConfig {
    pub jwt_secret: String,
    pub jwt_expiration_hours: i64,
    pub max_login_attempts: i32,
    pub lock_duration_minutes: i32,
    pub bcrypt_cost: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LogConfig {
    pub level: String,
    pub file_path: Option<String>,
    #[serde(default)]
    pub retention_days: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct PlatformConfig {
    #[serde(default = "default_platform_target")]
    pub target: String,
    #[serde(default = "default_service_name")]
    pub service_name: String,
    #[serde(default = "default_install_dir")]
    pub install_dir: String,
    #[serde(default = "default_service_account")]
    pub service_account: String,
}

fn default_platform_target() -> String {
    if cfg!(target_os = "windows") {
        "windows".to_string()
    } else {
        "linux".to_string()
    }
}
fn default_service_name() -> String { "HTKIS-ShareWeb".to_string() }
fn default_install_dir() -> String { "C:\\HTKIS\\ShareWeb".to_string() }
fn default_service_account() -> String { "HTKIS_SvcShareWeb".to_string() }

#[derive(Debug, Clone, Deserialize)]
pub struct SecurityConfig {
    #[serde(default)]
    pub share_roots: Vec<String>,
    #[serde(default = "default_force_admin_setup")]
    pub force_admin_setup: bool,
    #[serde(default = "default_log_retention")]
    pub log_retention_days: u32,
    #[serde(default)]
    pub tls_cert: Option<String>,
    #[serde(default)]
    pub tls_key: Option<String>,
    #[serde(default = "default_tls_min_version")]
    pub tls_min_version: String,
    #[serde(default = "default_jwt_cookie_only")]
    pub jwt_cookie_only: bool,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        SecurityConfig {
            share_roots: Vec::new(),
            force_admin_setup: true,
            log_retention_days: 30,
            tls_cert: None,
            tls_key: None,
            tls_min_version: "1.2".to_string(),
            jwt_cookie_only: false,
        }
    }
}

fn default_force_admin_setup() -> bool { true }
fn default_log_retention() -> u32 { 30 }
fn default_tls_min_version() -> String { "1.2".to_string() }
fn default_jwt_cookie_only() -> bool { false }

impl AppConfig {
    pub fn load(path: &str) -> Result<Self, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("读取配置文件失败: {}", e))?;
        let mut config: AppConfig = toml::from_str(&content)
            .map_err(|e| format!("解析配置文件失败: {}", e))?;

        if let Ok(secret) = env::var("JWT_SECRET") {
            config.auth.jwt_secret = secret;
        }
        if let Ok(base_dir) = env::var("STORAGE_BASE_DIR") {
            config.storage.base_dir = base_dir;
        }
        if let Ok(db_path) = env::var("DATABASE_PATH") {
            config.database.path = db_path;
        }

        if config.security.share_roots.is_empty() {
            config.security.share_roots.push(config.storage.base_dir.clone());
        }

        Ok(config)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.auth.jwt_secret.is_empty() {
            return Err("jwt_secret 不能为空".to_string());
        }
        if self.storage.base_dir.is_empty() {
            return Err("storage.base_dir 不能为空".to_string());
        }
        if self.database.path.is_empty() {
            return Err("database.path 不能为空".to_string());
        }
        if self.auth.bcrypt_cost < 4 || self.auth.bcrypt_cost > 31 {
            return Err("bcrypt_cost 必须在 4-31 之间".to_string());
        }
        if self.server.port == 0 {
            return Err("server.port 不能为 0".to_string());
        }
        if cfg!(target_os = "windows") && self.security.share_roots.is_empty() {
            return Err("Windows 平台必须配置 security.share_roots".to_string());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_security_config() {
        let s = SecurityConfig::default();
        assert!(s.force_admin_setup);
        assert_eq!(s.log_retention_days, 30);
        assert_eq!(s.tls_min_version, "1.2");
    }

    #[test]
    fn test_platform_config_defaults() {
        let p = PlatformConfig::default();
        assert!(!p.service_name.is_empty());
    }
}
