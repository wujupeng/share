#[cfg(target_os = "windows")]
pub mod service;

#[cfg(target_os = "windows")]
pub use service::run_as_service;