#[cfg(windows)]
mod service;

#[cfg(windows)]
mod utils;

use clap::Parser;

/*
    TODO:
    Trace/log file auto rotation
    https://docs.rs/tracing-appender/latest/tracing_appender/index.html -
*/

const SERVICE_NAME: &str = "ip_on_ondrive_service";
const SERVICE_DISPLAY_NAME: &str = "IP on File Service";
const SERVICE_DISCRIPTION: &str = "Windows Service to put IP list in a file for IP Discovery";

#[derive(Parser, Debug)]
#[clap(name = SERVICE_NAME, about = SERVICE_DISCRIPTION)]
struct Opt {
    #[clap(short = 'i', long = "install", default_value_t = false)]
    install: bool,

    #[clap(short = 'u', long = "uninstall", default_value_t = false)]
    uninstall: bool,

    #[clap(short = 'r', long = "restart", default_value_t = false)]
    restart: bool,

    #[clap(short = 'l', long = "log")]
    log_file: Option<String>,

    #[clap(short = 'o', long = "output")]
    ip_log_file: Option<String>,

    #[clap(short = 't', long = "time")]
    time_delay: Option<u64>,
}

#[cfg(windows)]
fn main() -> windows_service::Result<()> {
    let opt = Opt::parse();

    utils::set_default_log_path(SERVICE_NAME)?;
    if let Some(log_file_path) = opt.log_file.clone() {
        utils::set_log_path(SERVICE_NAME, &log_file_path)?;
    }

    utils::set_default_ip_log_path(SERVICE_NAME)?;
    if let Some(log_file_path) = opt.ip_log_file.clone() {
        utils::set_ip_log_path(SERVICE_NAME, &log_file_path)?;
    }

    let log_path: Option<&str>;
    let log_path_holder;
    if let Ok(Some(log_path_val)) = utils::get_log_path(SERVICE_NAME) {
        log_path_holder = log_path_val;
        log_path = Some(&log_path_holder);
    } else {
        log_path = None;
    }

    if let Some(td) = opt.time_delay {
        utils::set_time_delay(SERVICE_NAME, td)?;
    }

    let time_delay = utils::get_time_delay(SERVICE_NAME)?;

    utils::logging(log_path)?;

    tracing::info!("{}", SERVICE_NAME);

    if opt.install {
        tracing::info!("Installing Service");
        service::install_service(
            "ip_to_file.exe",
            SERVICE_NAME,
            SERVICE_DISPLAY_NAME,
            SERVICE_DISCRIPTION,
        )
    } else if opt.uninstall {
        tracing::info!("Uninstalling Service");
        service::uninstall_service(SERVICE_NAME)
    } else if opt.restart {
        tracing::info!("Restarting Service");
        service::restart_service(SERVICE_NAME)
    } else if opt.log_file.is_some() || opt.time_delay.is_some() || opt.ip_log_file.is_some()
    {
        // No other action to take
        Ok(())
    } else {
        tracing::info!("Running Service");
        service::run(SERVICE_NAME, time_delay)
    }
}

#[cfg(not(windows))]
fn main() {
    panic!("This program is only intended to run on Windows.");
}
