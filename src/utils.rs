use registry::{Hive, RegKey, Security};
use std::fs::OpenOptions;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Layer};
use utfx::U16CString;

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

pub fn set_ip_log_path(service_name: &str, log_file_path: &str) -> windows_service::Result<()> {
    let regkey = get_service_reg_key(service_name)?;

    let value = registry::Data::String(U16CString::from_str(log_file_path).unwrap());

    regkey.set_value("ip_log", &value).unwrap();

    Ok(())
}
