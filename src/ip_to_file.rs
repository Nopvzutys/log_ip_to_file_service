use std::{
    ffi::OsString,
    io::Write,
    net::IpAddr,
    sync::{mpsc, LazyLock, Mutex},
    time::Duration,
};
use windows_service::{
    define_windows_service,
    service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
    service_dispatcher, Result,
};

const SERVICE_TYPE: ServiceType = ServiceType::OWN_PROCESS;

static SERVICE_NAME: LazyLock<Mutex<String>> = LazyLock::new(|| Mutex::new(String::default()));
static POLL_RATE: LazyLock<Mutex<u64>> = LazyLock::new(|| Mutex::new(15 * 60));

pub fn run(service_name: &str, poll_rate: Option<u64>) -> Result<()> {
    {
        let mut lock = SERVICE_NAME.lock().unwrap();
        *lock = service_name.to_owned();
    }

    if poll_rate.is_some() {
        let mut lock = POLL_RATE.lock().unwrap();
        *lock = poll_rate.unwrap();
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
