#[cfg(windows)]
mod service;

#[cfg(windows)]
mod utils;

use clap::Parser;

const SERVICE_NAME: &str = "ip_to_file_service";
const SERVICE_DISPLAY_NAME: &str = "IP to File Service";
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

    #[clap(short = 'v', long = "verbose", default_value_t = false)]
    verbose: bool,
}

#[cfg(windows)]
fn main() -> windows_service::Result<()> {
    let opt = Opt::parse();

    println!("Setting default log path");
    if let Err(e) = utils::set_default_log_path(SERVICE_NAME) {
        if !opt.install {
            eprintln!("Error setting default log path: {} {}", SERVICE_NAME, e);
            return Err(windows_service::Error::Winapi(std::io::Error::new(
                std::io::ErrorKind::Other,
                e,
            )));
        }
    }
    if let Some(log_file_path) = opt.log_file.clone() {
        if let Err(e) = utils::set_log_path(SERVICE_NAME, &log_file_path) {
            if !opt.install {
                eprintln!(
                    "Error setting log path: {} {} {}",
                    SERVICE_NAME, &log_file_path, e
                );
                return Err(windows_service::Error::Winapi(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e,
                )));
            }
        }
    }

    utils::set_default_ip_log_path(SERVICE_NAME)?;
    println!("Setting default ip log path");
    if let Err(e) = utils::set_default_ip_log_path(SERVICE_NAME) {
        if !opt.install {
            eprintln!("Error setting default ip log path: {} {}", SERVICE_NAME, e);
            return Err(windows_service::Error::Winapi(std::io::Error::new(
                std::io::ErrorKind::Other,
                e,
            )));
        }
    }
    if let Some(log_file_path) = opt.ip_log_file.clone() {
        utils::set_ip_log_path(SERVICE_NAME, &log_file_path)?;
        if let Err(e) = utils::set_ip_log_path(SERVICE_NAME, &log_file_path) {
            if !opt.install {
                eprintln!(
                    "Error setting ip log path: {} {} {}",
                    SERVICE_NAME, &log_file_path, e
                );
                return Err(windows_service::Error::Winapi(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e,
                )));
            }
        }
    }

    println!("Getting log path");
    let log_path: Option<&str>;
    let log_path_holder;
    if let Ok(Some(log_path_val)) = utils::get_log_path(SERVICE_NAME) {
        log_path_holder = log_path_val;
        log_path = Some(&log_path_holder);
    } else {
        log_path = None;
    }

    println!("Setting time delay");

    if let Some(td) = opt.time_delay {
        if let Err(e) = utils::set_time_delay(SERVICE_NAME, td) {
            if !opt.install {
                eprintln!("Error setting time delay: {} {} {}", SERVICE_NAME, td, e);
                return Err(windows_service::Error::Winapi(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e,
                )));
            }
        }
    }

    println!("Getting time delay");
    let time_delay_res = utils::get_time_delay(SERVICE_NAME);
    let mut time_delay = None;
    if let Err(e) = time_delay_res {
        if !opt.install {
            eprintln!("Error getting time delay: {} {}", SERVICE_NAME, e);
            return Err(windows_service::Error::Winapi(std::io::Error::new(
                std::io::ErrorKind::Other,
                e,
            )));
        }
    } else {
        time_delay = time_delay_res.unwrap();
    }

    println!("Logging");
    if let Err(e) = utils::logging(log_path) {
        eprintln!("Error logging: {} {}", SERVICE_NAME, e);
    }

    println!("Installing Service");
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
    } else if opt.log_file.is_some() || opt.time_delay.is_some() || opt.ip_log_file.is_some() {
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
