use std::sync::OnceLock;
use tokio_util::sync::CancellationToken;

use crate::config::AppConfig;

static CONFIG_PATH: OnceLock<String> = OnceLock::new();

windows_service::define_windows_service!(ffi_service_main, my_service_main);

fn my_service_main(_arguments: Vec<std::ffi::OsString>) {
    use windows_service::service::ServiceControl;
    use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
    use windows_service::service::{ServiceState, ServiceStatus, ServiceControlAccept};

    let config_path = CONFIG_PATH.get().map(|s| s.as_str()).unwrap_or("config.windows.toml");

    std::fs::OpenOptions::new()
        .create(true).append(true)
        .open("C:\\service_debug.log").ok()
        .and_then(|mut f| std::io::Write::write_all(&mut f, b"my_service_main entered\n").ok());

    let cancel_token = CancellationToken::new();
    let cancel_for_handler = cancel_token.clone();

    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Stop => {
                cancel_for_handler.cancel();
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    let status_handle = match service_control_handler::register("HTKIS-ShareWeb", event_handler) {
        Ok(h) => h,
        Err(e) => {
            std::fs::OpenOptions::new()
                .create(true).append(true)
                .open("C:\\service_debug.log").ok()
                .and_then(|mut f| std::io::Write::write_all(&mut f, format!("register failed: {}\n", e).as_bytes()).ok());
            return;
        }
    };

    std::fs::OpenOptions::new()
        .create(true).append(true)
        .open("C:\\service_debug.log").ok()
        .and_then(|mut f| std::io::Write::write_all(&mut f, b"register OK\n").ok());

    if let Err(e) = status_handle.set_service_status(ServiceStatus {
        service_type: windows_service::service::ServiceType::OWN_PROCESS,
        current_state: ServiceState::StartPending,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: windows_service::service::ServiceExitCode::NO_ERROR,
        checkpoint: 1,
        wait_hint: std::time::Duration::from_secs(30),
        process_id: None,
    }) {
        std::fs::OpenOptions::new()
            .create(true).append(true)
            .open("C:\\service_debug.log").ok()
            .and_then(|mut f| std::io::Write::write_all(&mut f, format!("set_start_pending failed: {}\n", e).as_bytes()).ok());
        return;
    }

    let config = match AppConfig::load(config_path) {
        Ok(c) => c,
        Err(e) => {
            std::fs::OpenOptions::new()
                .create(true).append(true)
                .open("C:\\service_debug.log").ok()
                .and_then(|mut f| std::io::Write::write_all(&mut f, format!("config load failed: {}\n", e).as_bytes()).ok());
            let _ = status_handle.set_service_status(ServiceStatus {
                service_type: windows_service::service::ServiceType::OWN_PROCESS,
                current_state: ServiceState::Stopped,
                controls_accepted: ServiceControlAccept::empty(),
                exit_code: windows_service::service::ServiceExitCode::ServiceSpecific(1),
                checkpoint: 0,
                wait_hint: std::time::Duration::from_secs(0),
                process_id: None,
            });
            return;
        }
    };
    if let Err(e) = config.validate() {
        std::fs::OpenOptions::new()
            .create(true).append(true)
            .open("C:\\service_debug.log").ok()
            .and_then(|mut f| std::io::Write::write_all(&mut f, format!("config validate failed: {}\n", e).as_bytes()).ok());
        return;
    }

    std::fs::OpenOptions::new()
        .create(true).append(true)
        .open("C:\\service_debug.log").ok()
        .and_then(|mut f| std::io::Write::write_all(&mut f, b"config OK, setting Running\n").ok());

    if let Err(e) = status_handle.set_service_status(ServiceStatus {
        service_type: windows_service::service::ServiceType::OWN_PROCESS,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP,
        exit_code: windows_service::service::ServiceExitCode::NO_ERROR,
        checkpoint: 0,
        wait_hint: std::time::Duration::from_secs(10),
        process_id: None,
    }) {
        std::fs::OpenOptions::new()
            .create(true).append(true)
            .open("C:\\service_debug.log").ok()
            .and_then(|mut f| std::io::Write::write_all(&mut f, format!("set_running failed: {}\n", e).as_bytes()).ok());
        return;
    }

    std::fs::OpenOptions::new()
        .create(true).append(true)
        .open("C:\\service_debug.log").ok()
        .and_then(|mut f| std::io::Write::write_all(&mut f, b"Running OK, starting server\n").ok());

    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            std::fs::OpenOptions::new()
                .create(true).append(true)
                .open("C:\\service_debug.log").ok()
                .and_then(|mut f| std::io::Write::write_all(&mut f, format!("runtime failed: {}\n", e).as_bytes()).ok());
            return;
        }
    };

    let result = rt.block_on(crate::run_server(config_path, Some(cancel_token)));

    std::fs::OpenOptions::new()
        .create(true).append(true)
        .open("C:\\service_debug.log").ok()
        .and_then(|mut f| std::io::Write::write_all(&mut f, format!("run_server returned: {:?}\n", result.is_ok()).as_bytes()).ok());

    let _ = status_handle.set_service_status(ServiceStatus {
        service_type: windows_service::service::ServiceType::OWN_PROCESS,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: match result {
            Ok(()) => windows_service::service::ServiceExitCode::NO_ERROR,
            Err(_) => windows_service::service::ServiceExitCode::ServiceSpecific(1),
        },
        checkpoint: 0,
        wait_hint: std::time::Duration::from_secs(0),
        process_id: None,
    });
}

pub fn run_as_service(config_path: &str) -> std::io::Result<()> {
    std::fs::write("C:\\service_debug.log", "run_as_service entered\n").ok();

    let _ = CONFIG_PATH.set(config_path.to_string());

    windows_service::service_dispatcher::start("HTKIS-ShareWeb", ffi_service_main)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("服务调度器启动失败: {}", e)))?;

    std::fs::OpenOptions::new()
        .create(true).append(true)
        .open("C:\\service_debug.log").ok()
        .and_then(|mut f| std::io::Write::write_all(&mut f, b"dispatcher returned\n").ok());

    Ok(())
}