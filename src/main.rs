mod config;
mod error;
mod filesystem;
mod handlers;
mod middleware;
mod models;
mod repositories;
mod services;
mod utils;
#[cfg(target_os = "windows")]
mod windows;

use std::sync::{Mutex, Arc};
use std::sync::atomic::{AtomicBool, Ordering};

use actix_cors::Cors;
use actix_web::{web, App, HttpServer, middleware as actix_middleware, HttpResponse};
use rusqlite::Connection;
use tracing_subscriber::{fmt, EnvFilter};

use crate::config::AppConfig;
use crate::handlers::{auth, health, files, upload, download, preview, users, permissions, audit, setup};
use crate::middleware::setup_guard::SetupGuard;
use crate::repositories::user_repo::UserRepo;
use crate::services::auth_service::AuthService;
use crate::services::file_service::FileService;

fn apply_migration(conn: &Connection, version: &str, sql: &str) -> Result<(), String> {
    let already: i32 = conn
        .query_row(
            "SELECT COUNT(*) FROM schema_migrations WHERE version = ?1",
            rusqlite::params![version],
            |row| row.get(0),
        )
        .unwrap_or(0);
    if already > 0 {
        return Ok(());
    }
    conn.execute_batch(sql)
        .map_err(|e| format!("执行迁移 {} 失败: {}", version, e))?;
    conn.execute(
        "INSERT OR IGNORE INTO schema_migrations (version) VALUES (?1)",
        rusqlite::params![version],
    )
    .map_err(|e| format!("记录迁移版本 {} 失败: {}", version, e))?;
    tracing::info!("已应用迁移: {}", version);
    Ok(())
}

fn init_database(config: &AppConfig) -> Result<Arc<Mutex<Connection>>, String> {
    if let Some(parent) = std::path::Path::new(&config.database.path).parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建数据库目录失败: {}", e))?;
    }

    let conn = Connection::open(&config.database.path)
        .map_err(|e| format!("打开数据库失败: {}", e))?;

    let migration_001 = include_str!("../migrations/001_init.sql");
    let migration_002 = include_str!("../migrations/002_setup_state.sql");

    conn.execute_batch(migration_001)
        .map_err(|e| format!("初始化 schema_migrations 失败: {}", e))?;

    apply_migration(&conn, "001", migration_001)?;
    apply_migration(&conn, "002", migration_002)?;

    tracing::info!("数据库初始化完成: {}", config.database.path);
    Ok(Arc::new(Mutex::new(conn)))
}

fn is_admin_initialized(conn: &Connection) -> bool {
    let v: i32 = conn
        .query_row(
            "SELECT admin_initialized FROM setup_state WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    v != 0
}

fn run_migrate(config_path: &str) -> Result<(), String> {
    let config = AppConfig::load(config_path)?;
    config.validate()?;
    let db = init_database(&config)?;
    let conn = db.lock().unwrap();
    tracing::info!("迁移完成，已应用版本: 001, 002");
    let _ = conn;
    Ok(())
}

fn default_config_path() -> String {
    if cfg!(target_os = "windows") {
        "config.windows.toml".to_string()
    } else {
        "config.toml".to_string()
    }
}

pub async fn run_server(
    config_path: &str,
    cancel_token: Option<tokio_util::sync::CancellationToken>,
) -> std::io::Result<()> {
    let config = AppConfig::load(config_path).unwrap_or_else(|e| {
        eprintln!("配置加载失败: {}", e);
        std::process::exit(1);
    });

    if let Err(e) = config.validate() {
        eprintln!("配置校验失败: {}", e);
        std::process::exit(1);
    }

    fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new(&config.log.level)),
        )
        .init();

    tracing::info!("启动 Web 文件管理系统");
    tracing::info!("监听 {}:{}", config.server.host, config.server.port);
    tracing::info!("共享目录: {}", config.storage.base_dir);

    let db = init_database(&config).unwrap_or_else(|e| {
        eprintln!("数据库初始化失败: {}", e);
        std::process::exit(1);
    });

    let setup_initialized = {
        let conn = db.lock().unwrap();
        is_admin_initialized(&conn)
    };
    let setup_flag = Arc::new(AtomicBool::new(setup_initialized));
    let force_setup = config.security.force_admin_setup;

    if !setup_initialized && force_setup {
        tracing::warn!("管理员尚未初始化，请通过 /api/setup/admin 设置管理员密码");
    }

    let auth_service = AuthService::new(db.clone(), config.clone());
    let file_service = FileService::new(&config.storage.base_dir);

    let bind_addr = format!("{}:{}", config.server.host, config.server.port);
    let config_data = web::Data::new(config.clone());
    let auth_data = web::Data::new(auth_service);
    let file_data = web::Data::new(file_service);
    let setup_flag_data = web::Data::new(setup_flag.clone());

    let static_dir = config.server.static_dir.clone();
    let static_dir_opt = static_dir
        .as_ref()
        .filter(|d| std::path::Path::new(d).exists());

    let server = HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header()
            .max_age(3600);

        let mut app = App::new()
            .wrap(cors)
            .wrap(actix_middleware::Logger::default())
            .wrap(SetupGuard::new(setup_flag.clone(), force_setup))
            .app_data(web::PayloadConfig::new(100 * 1024 * 1024).limit(100 * 1024 * 1024))
            .app_data(config_data.clone())
            .app_data(auth_data.clone())
            .app_data(file_data.clone())
            .app_data(setup_flag_data.clone())
            .app_data(web::Data::new(db.clone()))
            .service(
                web::scope("/api")
                    .route("/health", web::get().to(health::health_check))
                    .route("/setup/status", web::get().to(setup::setup_status))
                    .route("/setup/admin", web::post().to(setup::setup_admin))
                    .route("/auth/login", web::post().to(auth::login))
                    .route("/auth/logout", web::post().to(auth::logout))
                    .route("/auth/profile", web::get().to(auth::profile))
                    .route("/auth/password", web::put().to(auth::change_password))
                    .route("/files", web::get().to(files::list_files))
                    .route("/files/tree", web::get().to(files::get_tree))
                    .route("/files/search", web::get().to(files::search_files))
                    .route("/files/mkdir", web::post().to(files::mkdir))
                    .route("/files/rename", web::put().to(files::rename))
                    .route("/files/delete", web::delete().to(files::delete))
                    .route("/files/move", web::post().to(files::move_file))
                    .route("/files/copy", web::post().to(files::copy_file))
                    .route("/files/download", web::get().to(download::download_file))
                    .route("/files/download/batch", web::post().to(download::batch_download))
                    .route("/files/preview", web::get().to(preview::preview_file))
                    .route("/upload/init", web::post().to(upload::init_upload))
                    .route("/upload/chunk", web::post().to(upload::upload_chunk))
                    .route("/upload/complete", web::post().to(upload::complete_upload))
                    .route("/upload/status", web::get().to(upload::upload_status))
                    .route("/users", web::get().to(users::list_users))
                    .route("/users", web::post().to(users::create_user))
                    .route("/users/{username}", web::put().to(users::update_user))
                    .route("/users/{username}", web::delete().to(users::delete_user))
                    .route("/permissions", web::get().to(permissions::list_permissions))
                    .route("/permissions", web::post().to(permissions::create_permission))
                    .route("/permissions/{id}", web::delete().to(permissions::delete_permission))
                    .route("/audit-logs", web::get().to(audit::list_audit_logs)),
            );

        if let Some(dir) = &static_dir_opt {
            app = app.service(
                actix_files::Files::new("/", dir)
                    .index_file("index.html")
                    .show_files_listing(false),
            );
        } else {
            app = app.route("/", web::get().to(|| async {
                HttpResponse::Ok().body("ShareWeb API — 前端静态资源未配置")
            }));
        }

        app
    })
    .bind(&bind_addr)?;

    let handle = server.handle();
    let run_fut = server.run();

    match cancel_token {
        Some(token) => {
            tokio::select! {
                res = run_fut => { return res; }
                _ = token.cancelled() => {
                    tracing::info!("收到停止信号，优雅关闭...");
                    handle.stop(true).await;
                    tracing::info!("服务已停止");
                }
            }
            Ok(())
        }
        None => run_fut.await,
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    let mut config_path: String = default_config_path();
    let mut mode = "console".to_string();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--service" => mode = "service".to_string(),
            "--console" => mode = "console".to_string(),
            "--migrate" => mode = "migrate".to_string(),
            arg if !arg.starts_with("--") => config_path = arg.to_string(),
            _ => {}
        }
        i += 1;
    }

    match mode.as_str() {
        "migrate" => {
            if let Err(e) = run_migrate(&config_path) {
                eprintln!("迁移失败: {}", e);
                std::process::exit(1);
            }
            println!("迁移完成");
            Ok(())
        }
        #[cfg(target_os = "windows")]
        "service" => windows::run_as_service(&config_path),
        #[cfg(not(target_os = "windows"))]
        "service" => {
            eprintln!("服务模式仅在 Windows 平台可用");
            std::process::exit(1);
        }
        _ => run_server(&config_path, None).await,
    }
}
