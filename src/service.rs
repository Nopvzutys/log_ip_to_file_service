use std::{
    ffi::OsString,
    io::Write,
    net::IpAddr,
    sync::{mpsc, LazyLock, Mutex},
    time::{Duration, Instant},
};
use windows_service::{
    define_windows_service,
    service::{
        ServiceAccess, ServiceControl, ServiceControlAccept, ServiceErrorControl, ServiceExitCode,
        ServiceInfo, ServiceStartType, ServiceState, ServiceStatus, ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
    service_dispatcher,
    service_manager::{ServiceManager, ServiceManagerAccess},
    Result,
};
use windows_sys::Win32::Foundation::ERROR_SERVICE_DOES_NOT_EXIST;

const SERVICE_TYPE: ServiceType = ServiceType::OWN_PROCESS;

static SERVICE_NAME: LazyLock<Mutex<String>> = LazyLock::new(|| Mutex::new(String::default()));
static POLL_RATE: LazyLock<Mutex<u64>> = LazyLock::new(|| Mutex::new(15 * 60));

pub fn run(service_name: &str, poll_rate: Option<u64>) -> Result<()> {
    tracing::info!("Running service: {}", service_name);
    {
        let mut lock = match SERVICE_NAME.lock() {
            Ok(l) => l,
            Err(e) => {
                tracing::error!("Failed to lock SERVICE_NAME: {}", e);
                return Err(windows_service::Error::Winapi(std::io::Error::other(
                    e.to_string(),
                )));
            }
        };
        *lock = service_name.to_owned();
    }

    if let Some(poll_rate) = poll_rate {
        let mut lock = match POLL_RATE.lock() {
            Ok(l) => l,
            Err(e) => {
                tracing::error!("Failed to lock POLL_RATE: {}", e);
                return Err(windows_service::Error::Winapi(std::io::Error::other(
                    e.to_string(),
                )));
            }
        };
        *lock = poll_rate;
    }

    service_dispatcher::start(service_name, ffi_service_main)
}

define_windows_service!(ffi_service_main, my_service_main);
fn my_service_main(_arguments: Vec<OsString>) {
    if let Err(e) = run_service() {
        tracing::error!("Run service failed: {:#?}", e)
    }
}

fn run_service() -> Result<()> {
    let (shutdown_tx, shutdown_rx) = mpsc::channel();

    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            ServiceControl::Stop => {
                shutdown_tx.send(()).unwrap();
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::UserEvent(code) => {
                if code.to_raw() == 130 {
                    shutdown_tx.send(()).unwrap();
                }
                ServiceControlHandlerResult::NoError
            }
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };
    let service_name;
    {
        let lock = SERVICE_NAME.lock().unwrap();
        service_name = lock.clone();
    }
    let status_handle = service_control_handler::register(&service_name, event_handler)?;
    status_handle.set_service_status(ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    let mut ip_addr_hist = vec![];

    loop {
        let mut ip_addrs: Vec<IpAddr> = vec![];
        let mut keep_ip_addrs: Vec<IpAddr> = vec![];
        for adapter in ipconfig::get_adapters().unwrap() {
            ip_addrs.extend(adapter.ip_addresses().iter());
        }
        ip_addrs.sort();
        ip_addrs.dedup();
        for ip in ip_addrs.drain(..) {
            if ip.is_ipv4() && !ip.is_loopback() && !ip.is_multicast() {
                tracing::info!("IP: {}", ip);
                keep_ip_addrs.push(ip);
            }
        }
        ip_addr_hist.push(keep_ip_addrs);
        let len = ip_addr_hist.len();
        if len > 4 {
            ip_addr_hist.drain(0..len - 4);
        }

        {
            let lock = SERVICE_NAME.lock().unwrap();
            let odpath = super::utils::get_ip_log_path(&lock)?.unwrap();
            let mut file = std::fs::File::create(&odpath).unwrap();
            let content = format!("{:#?}", &ip_addr_hist);
            file.write_all(content.as_bytes()).unwrap();
        }

        match shutdown_rx.recv_timeout(Duration::from_secs(*POLL_RATE.lock().unwrap())) {
            Ok(_) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => (),
        };
    }

    // Tell the system that service has stopped.
    status_handle.set_service_status(ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    Ok(())
}

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

    tracing::info!("Delete service");
    service.delete()?;
    if service.query_status()?.current_state != ServiceState::Stopped {
        service.stop()?;
    }
    drop(service);

    tracing::info!("Wait for service deletion");

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

    tracing::info!("Restart service");
    service.stop()?;
    service.start(&[std::ffi::OsStr::new("Started from Rust!")])?;

    Ok(())
}
