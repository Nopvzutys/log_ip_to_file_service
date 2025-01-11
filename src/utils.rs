use registry::{Hive, RegKey, Security};
use std::fs::OpenOptions;
use std::time::Duration;
use std::{ffi::OsString, time::Instant};
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Layer};
use utfx::U16CString;
use windows_service::{
    service::{
        ServiceAccess, ServiceErrorControl, ServiceInfo, ServiceStartType, ServiceState,
        ServiceType,
    },
    service_manager::{ServiceManager, ServiceManagerAccess},
};
use windows_sys::Win32::Foundation::ERROR_SERVICE_DOES_NOT_EXIST;

pub fn install_service(
    service_exe_name: &str,
    service_name: &str,
    display_name: &str,
    description: &str,
) -> windows_service::Result<()> {
    let service_binary_path = ::std::env::current_exe()
        .unwrap()
        .with_file_name(service_exe_name);
    tracing::info!("Service Binary: {}", service_binary_path.display());

    tracing::info!("Connecting to Service Manager");
    let manager_access = ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE;
    let service_manager = ServiceManager::local_computer(None::<&str>, manager_access)?;

    let service_info = ServiceInfo {
        name: OsString::from(service_name),
        display_name: OsString::from(display_name),
        service_type: ServiceType::OWN_PROCESS,
        start_type: ServiceStartType::OnDemand,
        error_control: ServiceErrorControl::Normal,
        executable_path: service_binary_path,
        launch_arguments: vec![],
        dependencies: vec![],
        account_name: None,
        account_password: None,
    };

    tracing::info!("Create Service");
    let service = service_manager.create_service(&service_info, ServiceAccess::CHANGE_CONFIG)?;
    service.set_description(description)?;

    tracing::info!("Service Install complete");

    Ok(())
}

pub fn uninstall_service(service_name: &str) -> windows_service::Result<()> {
    tracing::info!("Connecting to Service Manager");
    let manager_access = ServiceManagerAccess::CONNECT;
    let service_manager = ServiceManager::local_computer(None::<&str>, manager_access)?;

    let service_access = ServiceAccess::QUERY_STATUS | ServiceAccess::STOP | ServiceAccess::DELETE;
    let service = service_manager.open_service(service_name, service_access)?;

    // The service will be marked for deletion as long as this function call succeeds.
    // However, it will not be deleted from the database until it is stopped and all open handles to it are closed.

    tracing::info!("Delete service");
    service.delete()?;
    // Our handle to it is not closed yet. So we can still query it.
    if service.query_status()?.current_state != ServiceState::Stopped {
        // If the service cannot be stopped, it will be deleted when the system restarts.
        service.stop()?;
    }
    // Explicitly close our open handle to the service. This is automatically called when `service` goes out of scope.
    drop(service);

    tracing::info!("Wait for service deletion");
    // Win32 API does not give us a way to wait for service deletion.
    // To check if the service is deleted from the database, we have to poll it ourselves.
    let start = Instant::now();
    let timeout = Duration::from_secs(5);
    while start.elapsed() < timeout {
        if let Err(windows_service::Error::Winapi(e)) =
            service_manager.open_service(service_name, ServiceAccess::QUERY_STATUS)
        {
            if e.raw_os_error() == Some(ERROR_SERVICE_DOES_NOT_EXIST as i32) {
                return Ok(());
            }
        }
        std::thread::sleep(Duration::from_secs(1));
    }

    tracing::info!("Uninstalled service");
    Ok(())
}

pub fn restart_service(service_name: &str) -> windows_service::Result<()> {
    tracing::info!("Connecting to Service Manager");
    let manager_access = ServiceManagerAccess::CONNECT;
    let service_manager = ServiceManager::local_computer(None::<&str>, manager_access)?;

    let service_access = ServiceAccess::QUERY_STATUS | ServiceAccess::STOP | ServiceAccess::DELETE;
    let service = service_manager.open_service(service_name, service_access)?;

    // The service will be marked for deletion as long as this function call succeeds.
    // However, it will not be deleted from the database until it is stopped and all open handles to it are closed.

    tracing::info!("Restart service");
    service.stop()?;
    service.start(&[std::ffi::OsStr::new("Started from Rust!")])?;

    Ok(())
}

pub fn logging(log_file_path_opt: Option<&str>) -> windows_service::Result<()> {
    // enable console logging
    #[cfg(debug_assertions)]
    let log_level = LevelFilter::DEBUG;
    #[cfg(not(debug_assertions))]
    let log_level = LevelFilter::INFO;

    let mut layers = Vec::new();
    let layer = tracing_subscriber::fmt::layer()
        .with_timer(tracing_subscriber::fmt::time::LocalTime::rfc_3339())
        .with_filter(log_level)
        // Box the layer as a type-erased trait object, so that it can
        // be pushed to the `Vec`.
        .boxed();

    layers.push(layer);

    if let Some(log_file_path) = log_file_path_opt {
        let log_file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(log_file_path)
            .unwrap();

        let layer = tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_writer(log_file)
            .boxed();

        layers.push(layer);
    }

    tracing_subscriber::registry().with(layers).init();

    Ok(())
}

fn get_service_reg_key(service_name: &str) -> windows_service::Result<RegKey> {
    let regpath = format!("SYSTEM\\CurrentControlSet\\Services\\{}", service_name);

    let regkey = Hive::LocalMachine.open(&regpath, Security::Write | Security::Read);

    match regkey {
        Ok(r) => Ok(r),
        Err(e) => match e {
            registry::key::Error::InvalidNul(_) => Err(windows_service::Error::Winapi(
                std::io::Error::from_raw_os_error(87),
            )),
            registry::key::Error::NotFound(_, n) => Err(windows_service::Error::Winapi(n)),
            registry::key::Error::PermissionDenied(_, n) => Err(windows_service::Error::Winapi(n)),
            registry::key::Error::Unknown(_, n) => Err(windows_service::Error::Winapi(n)),
            _ => Err(windows_service::Error::Winapi(
                std::io::Error::from_raw_os_error(87),
            )),
        },
    }
}

pub fn get_log_path(service_name: &str) -> windows_service::Result<Option<String>> {
    if let Ok(regkey) = get_service_reg_key(service_name) {
        if let Ok(registry::Data::String(s)) = regkey.value("log") {
            return Ok(Some(s.to_string().unwrap()));
        }
    }
    Ok(None)
}

pub fn set_default_log_path(service_name: &str) -> windows_service::Result<()> {
    if get_log_path(service_name).is_err() {
        let log_file_path = format!("{}.log.txt", service_name);
        set_log_path(service_name, &log_file_path)?;
    }

    Ok(())
}

pub fn set_log_path(service_name: &str, log_file_path: &str) -> windows_service::Result<()> {
    let regkey = get_service_reg_key(service_name)?;

    let value = registry::Data::String(U16CString::from_str(log_file_path).unwrap());

    regkey.set_value("log", &value).unwrap();

    Ok(())
}

pub fn set_time_delay(service_name: &str, time_delay: u64) -> windows_service::Result<()> {
    let regkey = get_service_reg_key(service_name)?;

    let value = registry::Data::U64(time_delay);

    regkey.set_value("time_delay", &value).unwrap();

    Ok(())
}

pub fn get_time_delay(service_name: &str) -> windows_service::Result<Option<u64>> {
    if let Ok(regkey) = get_service_reg_key(service_name) {
        if let Ok(registry::Data::U64(s)) = regkey.value("time_delay") {
            return Ok(Some(s));
        }
    }
    Ok(None)
}

pub fn get_ip_log_path(service_name: &str) -> windows_service::Result<Option<String>> {
    if let Ok(regkey) = get_service_reg_key(service_name) {
        if let Ok(registry::Data::String(s)) = regkey.value("ip_log") {
            return Ok(Some(s.to_string().unwrap()));
        }
    }
    Ok(None)
}

pub fn set_default_ip_log_path(service_name: &str) -> windows_service::Result<()> {
    if get_log_path(service_name).is_err() {
        let log_file_path = format!("{}.ip_log.txt", service_name);
        set_log_path(service_name, &log_file_path)?;
    }

    Ok(())
}

pub fn set_ip_log_path(
    service_name: &str,
    log_file_path: &str,
) -> windows_service::Result<()> {
    let regkey = get_service_reg_key(service_name)?;

    let value = registry::Data::String(U16CString::from_str(log_file_path).unwrap());

    regkey.set_value("ip_log", &value).unwrap();

    Ok(())
}
