use log::{error, info};
use miniserde::{Deserialize, Serialize};
use palc::{Parser, ValueEnum};
use std::fmt::Display;
use std::path::PathBuf;
use std::{env, fs};

#[derive(Parser, Debug, Clone)]
#[command(
    version,
    long_about = "komari-monitor-rs is a third-party high-performance monitoring agent for the komari monitoring service.",
    after_long_help = "Must set --http-server / --token\n--ip-provider accepts cloudflare / ipinfo\n--log-level accepts error, warn, info, debug, trace\n\nThis Agent is open-sourced on Github, powered by powerful Rust. Love from Komari"
)]
pub struct Args {
    // Main
    /// Set Main Server Http Address
    #[arg(long)]
    pub http_server: Option<String>,

    /// Set Main Server WebSocket Address
    #[arg(long)]
    pub ws_server: Option<String>,

    /// Set Token
    #[arg(short, long, allow_hyphen_values = true)]
    pub token: Option<String>,

    /// Set Fake Multiplier
    #[arg(short, long, default_value_t = 1.0)]
    pub fake: f64,

    /// Enable TLS (default disabled)
    #[arg(long, default_value_t = false)]
    pub tls: bool,

    /// Ignore Certificate Verification
    #[arg(long, default_value_t = false)]
    pub ignore_unsafe_cert: bool,

    /// Dry Run
    #[arg(short, long, default_value_t = false)]
    pub dry_run: bool,

    /// Set Log Level (Enable Debug or Trace for issue reporting)
    #[arg(long, default_value_t = log_level())]
    pub log_level: LogLevel,

    // Other
    /// Public IP Provider
    #[arg(long, default_value_t=ip_provider())]
    pub ip_provider: IpProvider,

    /// Enable Terminal (default disabled)
    #[arg(long, default_value_t = false)]
    pub terminal: bool,

    /// Custom Terminal Entry
    #[arg(long, default_value_t = terminal_entry())]
    pub terminal_entry: String,

    /// Set Real-Time Info Upload Interval (ms)
    #[arg(long, default_value_t = 1000)]
    pub realtime_info_interval: u64,

    /// Disable Windows Toast Notification (Only Windows)
    #[arg(long, default_value_t = false)]
    pub disable_toast_notify: bool,

    // Network
    /// Disable Network Statistics
    #[arg(long, default_value_t = false)]
    pub disable_network_statistics: bool,

    /// Network Statistics Interval (s)
    #[arg(long, default_value_t = 10)]
    pub network_interval: u32,

    /// Day of month to reset traffic (1-31, uses last day if month doesn't have that day)
    #[arg(long, default_value_t = 1)]
    pub reset_day: u8,

    /// Traffic calibration for upload (bytes)
    #[arg(long, default_value_t = 0)]
    pub calibration_tx: u64,

    /// Traffic calibration for download (bytes)
    #[arg(long, default_value_t = 0)]
    pub calibration_rx: u64,

    /// Network Statistics Save Path
    #[arg(long)]
    pub network_save_path: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct NetworkConfig {
    pub disable_network_statistics: bool,
    pub network_interval: u32,
    pub reset_day: u8,
    pub calibration_tx: u64,
    pub calibration_rx: u64,
    pub network_save_path: String,
}

impl Args {
    pub fn par() -> Self {
        let mut args = Self::parse();
        unsafe {
            crate::get_info::network::DURATION = args.realtime_info_interval as f64;
        }
        if args.terminal_entry == "default" {
            args.terminal_entry = {
                if cfg!(windows) {
                    "cmd.exe".to_string()
                } else if fs::exists("/bin/bash").unwrap_or(false) {
                    "bash".to_string()
                } else {
                    "sh".to_string()
                }
            };
        }
        args
    }
    pub fn network_config(&self) -> NetworkConfig {
        let path = {
            if self.network_save_path.is_none() {
                if cfg!(windows) {
                    PathBuf::from(r"C:\komari-network.conf")
                        .to_string_lossy()
                        .to_string()
                } else {
                    let is_root = env::var("EUID")
                        .unwrap_or("999".to_string())
                        .parse::<i32>()
                        .unwrap_or(999)
                        == 0
                        || env::var("UID")
                            .unwrap_or("999".to_string())
                            .parse::<i32>()
                            .unwrap_or(999)
                            == 0;
                    let path = if is_root {
                        PathBuf::from("/etc/komari-network.conf")
                            .to_string_lossy()
                            .to_string()
                    } else {
                        let home = env::var("HOME").unwrap_or_else(|_| {
                            error!(
                                        "Failed to automatically determine Network Config save path, this feature will be disabled."
                                    );
                            String::from("")
                        });

                        PathBuf::from(home)
                            .join(".config/komari-network.conf")
                            .to_string_lossy()
                            .to_string()
                    };
                    info!(
                        "Automatically determined Network Config save path: {}",
                        path
                    );
                    path
                }
            } else {
                let path = PathBuf::from(self.network_save_path.as_ref().unwrap())
                    .to_string_lossy()
                    .to_string();
                info!("Using specified Network Config save path: {}", path);
                path
            }
        };

        let disable_network_statistics = if self.disable_network_statistics {
            true
        } else if !self.disable_network_statistics && path.is_empty() {
            false
        } else {
            false
        };

        NetworkConfig {
            disable_network_statistics,
            network_interval: self.network_interval,
            reset_day: self.reset_day.clamp(1, 31),
            calibration_tx: self.calibration_tx,
            calibration_rx: self.calibration_rx,
            network_save_path: path,
        }
    }
}

impl Display for Args {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Configuration:")?;

        if let Some(http_server) = &self.http_server {
            writeln!(f, "  HTTP Server: {}", http_server)?;
        }

        if let Some(ws_server) = &self.ws_server {
            writeln!(f, "  WebSocket Server: {}", ws_server)?;
        }

        if let Some(token) = &self.token {
            writeln!(f, "  Token: {}", token)?;
        }

        if self.fake != 1.0 {
            writeln!(f, "  Fake Multiplier: {}", self.fake)?;
        }

        if self.tls {
            writeln!(f, "  TLS Enabled: true")?;
        }

        if self.ignore_unsafe_cert {
            writeln!(f, "  Ignore Unsafe Certificates: true")?;
        }

        if self.dry_run {
            writeln!(f, "  Dry Run Mode: enabled")?;
        }

        writeln!(f, "  Log Level: {:?}", self.log_level)?;
        writeln!(f, "  IP Provider: {:?}", self.ip_provider)?;

        if self.terminal {
            writeln!(f, "  Terminal Enabled: true")?;
            writeln!(f, "  Terminal Entry: {}", self.terminal_entry)?;
        }

        writeln!(
            f,
            "  Real-time Info Interval: {} ms",
            self.realtime_info_interval
        )?;

        writeln!(
            f,
            "  Disable Windows Toast Notify: {}",
            self.disable_toast_notify
        )?;

        writeln!(
            f,
            "  Network Statistics: {}",
            if self.disable_network_statistics {
                "disabled"
            } else {
                "enabled"
            }
        )?;

        if !self.disable_network_statistics {
            writeln!(f, "    Interval: {} s", self.network_interval)?;
            writeln!(f, "    Reset Day: {} of each month", self.reset_day)?;
            writeln!(f, "    Calibration TX: {} bytes", self.calibration_tx)?;
            writeln!(f, "    Calibration RX: {} bytes", self.calibration_rx)?;
            if let Some(save_path) = &self.network_save_path {
                writeln!(f, "    Save Path: {}", save_path)?;
            }
        }

        Ok(())
    }
}

// Default Settings

fn terminal_entry() -> String {
    "default".to_string()
}

fn ip_provider() -> IpProvider {
    IpProvider::Ipinfo
}

#[derive(Debug, Clone, ValueEnum)]
pub enum IpProvider {
    Cloudflare,
    Ipinfo,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

fn log_level() -> LogLevel {
    LogLevel::Info
}
