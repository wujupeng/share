use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio_util::sync::CancellationToken;

use crate::config::AppConfig;

pub fn run_as_service(config_path: &str) -> std::io::Result<()> {
    use windows_service::service::{
        ServiceAccess, ServiceControlAccept, ServiceErrorControl,
        ServiceExitCode, ServiceInfo, ServiceStartType, ServiceState, ServiceType,
    };
    use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

    let manager = ServiceManager::local_computer(
        None::<&str>,
        ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE,
    )
    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("连接 SCM 失败: {}", e)))?;

    let service_name = "HTKIS-ShareWeb";
    let display_name = "HTKIS ShareWeb File Manager";

    let existing = manager.open_service(
        service_name,
        ServiceAccess::QUERY_STATUS | ServiceAccess::DELETE,
    );

    if existing.is_ok() {
        tracing::info!("服务已注册，跳过创建");
    } else {
        let service_info = ServiceInfo {
            name: service_name.into(),
            display_name: display_name.into(),
            service_type: ServiceType::OWN_PROCESS,
            start_type: ServiceStartType::AutoStart,
            error_control: ServiceErrorControl::Normal,
            executable_path: std::env::current_exe()?,
            launch_arguments: vec!["--service".into(), config_path.into()],
            dependencies: vec![],
            account_name: None,
            account_password: None,
        };
        manager
            .create_service(&service_info, ServiceAccess::QUERY_STATUS)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("创建服务失败: {}", e)))?;
        tracing::info!("服务注册成功: {}", service_name);
    }

    run_service_dispatch(config_path)
}

fn run_service_dispatch(config_path: &str) -> std::io::Result<()> {
    use windows_service::service::ServiceControl;
    use windows_service::service_control_handler::{self, ServiceControlHandlerResult};

    let cancel_token = CancellationToken::new();
    let cancel_for_handler = cancel_token.clone();

    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            windows_service::service::ServiceControl::Stop => {
                cancel_for_handler.cancel();
                ServiceControlHandlerResult::NoError
            }
            windows_service::service::ServiceControl::Interrogate => {
                ServiceControlHandlerResult::NoError
            }
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    let status_handle = service_control_handler::register("HTKIS-ShareWeb", event_handler)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("注册控制处理器失败: {}", e)))?;

    use windows_service::service::{ServiceState, ServiceStatus, ServiceControlAccept};
    status_handle
        .set_service_status(ServiceStatus {
            service_type: windows_service::service::ServiceType::OWN_PROCESS,
            current_state: ServiceState::Running,
            controls_accepted: ServiceControlAccept::STOP,
            exit_code: windows_service::service::ServiceExitCode::NO_ERROR,
            checkpoint: 0,
            wait_hint: std::time::Duration::from_secs(10),
            process_id: None,
        })
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("设置运行状态失败: {}", e)))?;

    let config = AppConfig::load(config_path)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    if let Err(e) = config.validate() {
        return Err(std::io::Error::new(std::io::ErrorKind::Other, e));
    }

    let rt = tokio::runtime::Runtime::new()?;
    let result = rt.block_on(crate::run_server(config_path, Some(cancel_token)));

    status_handle
        .set_service_status(ServiceStatus {
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
        })
        .ok();

    result
}