use std::path::Path;
use tracing::level_filters::LevelFilter;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Layer};

pub fn logging(log_file_path_opt: Option<&str>) -> windows_service::Result<()> {
    #[cfg(debug_assertions)]
    let log_level = LevelFilter::DEBUG;
    #[cfg(not(debug_assertions))]
    let log_level = LevelFilter::INFO;

    let mut layers = Vec::new();
    let layer = tracing_subscriber::fmt::layer()
        .with_timer(tracing_subscriber::fmt::time::LocalTime::rfc_3339())
        .with_filter(log_level)
        .boxed();

    layers.push(layer);

    if let Some(log_file_path) = log_file_path_opt {
        let path = Path::new(log_file_path);
        let dir = path.parent().unwrap();
        let file = path.file_name().unwrap();

        let file_appender = RollingFileAppender::new(Rotation::DAILY, dir, file);

        let layer = tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_writer(file_appender)
            .boxed();

        layers.push(layer);
    }

    tracing_subscriber::registry().with(layers).init();

    Ok(())
}

fn get_service_reg_key(service_name: &str) -> windows_service::Result<windows_registry::Key> {
    let regpath = format!("SYSTEM\\CurrentControlSet\\Services\\{}", service_name);

    match windows_registry::LOCAL_MACHINE
        .options()
        .read()
        .write()
        .open(regpath)
    {
        Err(e) => {
            let h = e.code().0;
            tracing::error!("get_service_reg_key failed {}", &e.message());
            Err(windows_service::Error::Winapi(
                std::io::Error::from_raw_os_error(h),
            ))
        }
        Ok(k) => Ok(k),
    }
}

pub fn get_log_path(service_name: &str) -> windows_service::Result<Option<String>> {
    if let Ok(regkey) = get_service_reg_key(service_name) {
        if let Ok(s) = regkey.get_string("log") {
            return Ok(Some(s));
        }
    }
    Err(windows_service::Error::Winapi(
        std::io::Error::last_os_error(),
    ))
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

    match regkey.set_string("log", log_file_path) {
        Err(e) => {
            let h: i32 = e.code().0;
            eprintln!("set_log_path failed {}", &e.message());
            Err(windows_service::Error::Winapi(
                std::io::Error::from_raw_os_error(h),
            ))
        }
        Ok(()) => Ok(()),
    }
}

pub fn set_time_delay(service_name: &str, time_delay: u64) -> windows_service::Result<()> {
    let regkey = get_service_reg_key(service_name)?;

    match regkey.set_u64("time_delay", time_delay) {
        Err(e) => {
            let h = e.code().0;
            eprintln!("set_time_delay failed {}", &e.message());
            Err(windows_service::Error::Winapi(
                std::io::Error::from_raw_os_error(h),
            ))
        }
        Ok(()) => Ok(()),
    }
}

pub fn get_time_delay(service_name: &str) -> windows_service::Result<Option<u64>> {
    if let Ok(regkey) = get_service_reg_key(service_name) {
        if let Ok(s) = regkey.get_u64("time_delay") {
            return Ok(Some(s));
        }
    }
    Ok(None)
}

pub fn get_ip_log_path(service_name: &str) -> windows_service::Result<Option<String>> {
    if let Ok(regkey) = get_service_reg_key(service_name) {
        if let Ok(s) = regkey.get_string("ip_log") {
            return Ok(Some(s));
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
    tracing::info!("set_ip_log_path++");

    let regkey = get_service_reg_key(service_name)?;

    match regkey.set_string("ip_log", log_file_path) {
        Err(e) => {
            let h = e.code().0;
            tracing::error!("set_ip_log_path failed {}", &e.message());
            Err(windows_service::Error::Winapi(
                std::io::Error::from_raw_os_error(h),
            ))
        }
        Ok(()) => Ok(()),
    }
}
