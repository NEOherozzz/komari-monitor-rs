use log::info;
use palc::ValueEnum;
use std::env;
use std::fs;
use std::path::PathBuf;

// ==================== Enums ====================

#[derive(Debug, Clone, ValueEnum, PartialEq)]
pub enum IpProvider {
    Cloudflare,
    Ipinfo,
}

impl IpProvider {
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "cloudflare" => Ok(IpProvider::Cloudflare),
            "ipinfo" => Ok(IpProvider::Ipinfo),
            _ => Err(format!("Invalid ip_provider value: {}", s)),
        }
    }

    #[allow(dead_code)]
    pub fn to_string(&self) -> String {
        match self {
            IpProvider::Cloudflare => "cloudflare".to_string(),
            IpProvider::Ipinfo => "ipinfo".to_string(),
        }
    }
}

#[derive(Debug, Clone, ValueEnum, PartialEq)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl LogLevel {
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "error" => Ok(LogLevel::Error),
            "warn" => Ok(LogLevel::Warn),
            "info" => Ok(LogLevel::Info),
            "debug" => Ok(LogLevel::Debug),
            "trace" => Ok(LogLevel::Trace),
            _ => Err(format!("Invalid log_level value: {}", s)),
        }
    }

    #[allow(dead_code)]
    pub fn to_string(&self) -> String {
        match self {
            LogLevel::Error => "error".to_string(),
            LogLevel::Warn => "warn".to_string(),
            LogLevel::Info => "info".to_string(),
            LogLevel::Debug => "debug".to_string(),
            LogLevel::Trace => "trace".to_string(),
        }
    }
}

#[derive(Debug, Clone, ValueEnum, PartialEq)]
pub enum TrafficMode {
    Both,
    TxOnly,
    RxOnly,
}

impl TrafficMode {
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "both" => Ok(TrafficMode::Both),
            "tx_only" => Ok(TrafficMode::TxOnly),
            "rx_only" => Ok(TrafficMode::RxOnly),
            _ => Err(format!("Invalid traffic_mode value: {}", s)),
        }
    }

    #[allow(dead_code)]
    pub fn to_string(&self) -> String {
        match self {
            TrafficMode::Both => "both".to_string(),
            TrafficMode::TxOnly => "tx_only".to_string(),
            TrafficMode::RxOnly => "rx_only".to_string(),
        }
    }
}

// ==================== User Configuration ====================

#[derive(Debug, Clone, PartialEq)]
pub struct UserConfig {
    // Main Server Configuration
    pub http_server: String,
    pub ws_server: Option<String>,
    pub token: String,

    // TLS Configuration
    pub tls: bool,
    pub ignore_unsafe_cert: bool,

    // Performance Configuration
    pub fake: f64,
    pub realtime_info_interval: u64,

    // Feature Configuration
    pub ip_provider: IpProvider,
    pub terminal: bool,
    pub terminal_entry: String,
    pub disable_toast_notify: bool,

    // Network Statistics Configuration
    pub disable_network_statistics: bool,
    pub network_interval: u32,
    pub reset_day: u8,
    pub calibration_tx: u64,
    pub calibration_rx: u64,
    pub traffic_mode: TrafficMode,

    // Logging Configuration
    pub log_level: LogLevel,
}

impl Default for UserConfig {
    fn default() -> Self {
        Self {
            http_server: String::new(),
            ws_server: None,
            token: String::new(),
            tls: false,
            ignore_unsafe_cert: false,
            fake: 1.0,
            realtime_info_interval: 1000,
            ip_provider: IpProvider::Ipinfo,
            terminal: false,
            terminal_entry: "default".to_string(),
            disable_toast_notify: false,
            disable_network_statistics: false,
            network_interval: 10,
            reset_day: 1,
            calibration_tx: 0,
            calibration_rx: 0,
            traffic_mode: TrafficMode::Both,
            log_level: LogLevel::Info,
        }
    }
}

impl UserConfig {
    /// Encode configuration to key=value format
    #[allow(dead_code)]
    pub fn encode(&self) -> String {
        let mut lines = Vec::new();

        lines.push("# Komari Monitor Agent Configuration".to_string());
        lines.push(String::new());
        lines.push("# ==================== Main Server Configuration ====================".to_string());
        lines.push(format!("http_server={}", self.http_server));
        lines.push(format!("ws_server={}", self.ws_server.as_ref().unwrap_or(&String::new())));
        lines.push(format!("token={}", self.token));
        lines.push(String::new());

        lines.push("# ==================== TLS Configuration ====================".to_string());
        lines.push(format!("tls={}", self.tls));
        lines.push(format!("ignore_unsafe_cert={}", self.ignore_unsafe_cert));
        lines.push(String::new());

        lines.push("# ==================== Performance Configuration ====================".to_string());
        lines.push(format!("fake={}", self.fake));
        lines.push(format!("realtime_info_interval={}", self.realtime_info_interval));
        lines.push(String::new());

        lines.push("# ==================== Feature Configuration ====================".to_string());
        lines.push(format!("ip_provider={}", self.ip_provider.to_string()));
        lines.push(format!("terminal={}", self.terminal));
        lines.push(format!("terminal_entry={}", self.terminal_entry));
        lines.push(format!("disable_toast_notify={}", self.disable_toast_notify));
        lines.push(String::new());

        lines.push("# ==================== Network Statistics Configuration ====================".to_string());
        lines.push(format!("disable_network_statistics={}", self.disable_network_statistics));
        lines.push(format!("network_interval={}", self.network_interval));
        lines.push(format!("reset_day={}", self.reset_day));
        lines.push(format!("calibration_tx={}", self.calibration_tx));
        lines.push(format!("calibration_rx={}", self.calibration_rx));
        lines.push(format!("traffic_mode={}", self.traffic_mode.to_string()));
        lines.push(String::new());

        lines.push("# ==================== Logging Configuration ====================".to_string());
        lines.push(format!("log_level={}", self.log_level.to_string()));

        lines.join("\n")
    }

    /// Decode configuration from key=value format
    pub fn decode(content: &str) -> Result<Self, String> {
        let mut config = UserConfig::default();

        for (line_num, line) in content.lines().enumerate() {
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Parse key=value
            let parts: Vec<&str> = line.splitn(2, '=').collect();
            if parts.len() != 2 {
                return Err(format!("Invalid line {} (expected key=value format): {}", line_num + 1, line));
            }

            let key = parts[0].trim();
            let value = parts[1].trim();

            match key {
                // Main Server Configuration
                "http_server" => config.http_server = value.to_string(),
                "ws_server" => config.ws_server = if value.is_empty() { None } else { Some(value.to_string()) },
                "token" => config.token = value.to_string(),

                // TLS Configuration
                "tls" => config.tls = parse_bool(value, key)?,
                "ignore_unsafe_cert" => config.ignore_unsafe_cert = parse_bool(value, key)?,

                // Performance Configuration
                "fake" => config.fake = parse_f64(value, key)?,
                "realtime_info_interval" => config.realtime_info_interval = parse_u64(value, key)?,

                // Feature Configuration
                "ip_provider" => config.ip_provider = IpProvider::from_str(value)?,
                "terminal" => config.terminal = parse_bool(value, key)?,
                "terminal_entry" => config.terminal_entry = value.to_string(),
                "disable_toast_notify" => config.disable_toast_notify = parse_bool(value, key)?,

                // Network Statistics Configuration
                "disable_network_statistics" => config.disable_network_statistics = parse_bool(value, key)?,
                "network_interval" => config.network_interval = parse_u32(value, key)?,
                "reset_day" => config.reset_day = parse_u8(value, key)?,
                "calibration_tx" => config.calibration_tx = parse_u64(value, key)?,
                "calibration_rx" => config.calibration_rx = parse_u64(value, key)?,
                "traffic_mode" => config.traffic_mode = TrafficMode::from_str(value)?,

                // Logging Configuration
                "log_level" => config.log_level = LogLevel::from_str(value)?,

                _ => {
                    return Err(format!("Unknown configuration key at line {}: {}", line_num + 1, key));
                }
            }
        }

        // Validate required parameters
        if config.http_server.is_empty() {
            return Err("Missing required parameter: http_server".to_string());
        }
        if config.token.is_empty() {
            return Err("Missing required parameter: token".to_string());
        }

        // Clamp reset_day to valid range
        config.reset_day = config.reset_day.clamp(1, 31);

        // Process terminal_entry default
        if config.terminal_entry == "default" {
            config.terminal_entry = get_default_terminal_entry();
        }

        Ok(config)
    }
}

// ==================== Runtime Data ====================

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeData {
    pub boot_id: String,
    pub boot_source_tx: u64,
    pub boot_source_rx: u64,
    pub current_boot_tx: u64,
    pub current_boot_rx: u64,
    pub accumulated_tx: u64,
    pub accumulated_rx: u64,
    pub last_reset_month: u8,
}

impl Default for RuntimeData {
    fn default() -> Self {
        Self {
            boot_id: String::new(),
            boot_source_tx: 0,
            boot_source_rx: 0,
            current_boot_tx: 0,
            current_boot_rx: 0,
            accumulated_tx: 0,
            accumulated_rx: 0,
            last_reset_month: 1,
        }
    }
}

impl RuntimeData {
    /// Encode runtime data to key=value format
    pub fn encode(&self) -> String {
        let mut lines = Vec::new();

        lines.push("# Komari Monitor Runtime Data".to_string());
        lines.push("# This file is automatically managed by the program. Do not modify manually.".to_string());
        lines.push(String::new());
        lines.push(format!("boot_id={}", self.boot_id));
        lines.push(format!("boot_source_tx={}", self.boot_source_tx));
        lines.push(format!("boot_source_rx={}", self.boot_source_rx));
        lines.push(format!("current_boot_tx={}", self.current_boot_tx));
        lines.push(format!("current_boot_rx={}", self.current_boot_rx));
        lines.push(format!("accumulated_tx={}", self.accumulated_tx));
        lines.push(format!("accumulated_rx={}", self.accumulated_rx));
        lines.push(format!("last_reset_month={}", self.last_reset_month));

        lines.join("\n")
    }

    /// Decode runtime data from key=value format
    pub fn decode(content: &str) -> Result<Self, String> {
        let mut data = RuntimeData::default();

        for (line_num, line) in content.lines().enumerate() {
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Parse key=value
            let parts: Vec<&str> = line.splitn(2, '=').collect();
            if parts.len() != 2 {
                return Err(format!("Invalid line {} (expected key=value format): {}", line_num + 1, line));
            }

            let key = parts[0].trim();
            let value = parts[1].trim();

            match key {
                "boot_id" => data.boot_id = value.to_string(),
                "boot_source_tx" => data.boot_source_tx = parse_u64(value, key)?,
                "boot_source_rx" => data.boot_source_rx = parse_u64(value, key)?,
                "current_boot_tx" => data.current_boot_tx = parse_u64(value, key)?,
                "current_boot_rx" => data.current_boot_rx = parse_u64(value, key)?,
                "accumulated_tx" => data.accumulated_tx = parse_u64(value, key)?,
                "accumulated_rx" => data.accumulated_rx = parse_u64(value, key)?,
                "last_reset_month" => data.last_reset_month = parse_u8(value, key)?,
                _ => {
                    return Err(format!("Unknown runtime data key at line {}: {}", line_num + 1, key));
                }
            }
        }

        Ok(data)
    }
}

// ==================== Configuration Paths ====================

pub struct ConfigPath;

impl ConfigPath {
    /// Get user configuration file path
    pub fn user_config(custom_path: Option<&str>) -> Result<PathBuf, String> {
        if let Some(path) = custom_path {
            return Ok(PathBuf::from(path));
        }

        if cfg!(windows) {
            Ok(PathBuf::from(r"C:\komari-agent.conf"))
        } else {
            let is_root = is_root_user();
            if is_root {
                Ok(PathBuf::from("/etc/komari-agent.conf"))
            } else {
                let home = env::var("HOME")
                    .map_err(|_| "Failed to get HOME environment variable".to_string())?;
                Ok(PathBuf::from(home).join(".config/komari-agent.conf"))
            }
        }
    }

    /// Get runtime data file path
    pub fn runtime_data() -> Result<PathBuf, String> {
        if cfg!(windows) {
            let path = PathBuf::from(r"C:\ProgramData\komari-monitor\network-data.conf");
            // Ensure parent directory exists
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create runtime data directory: {}", e))?;
            }
            Ok(path)
        } else {
            let is_root = is_root_user();
            if is_root {
                let path = PathBuf::from("/var/lib/komari-monitor/network-data.conf");
                // Ensure parent directory exists
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent)
                        .map_err(|e| format!("Failed to create runtime data directory: {}", e))?;
                }
                Ok(path)
            } else {
                let home = env::var("HOME")
                    .map_err(|_| "Failed to get HOME environment variable".to_string())?;
                let path = PathBuf::from(home).join(".local/share/komari-monitor/network-data.conf");
                // Ensure parent directory exists
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent)
                        .map_err(|e| format!("Failed to create runtime data directory: {}", e))?;
                }
                Ok(path)
            }
        }
    }
}

// ==================== Configuration Reader ====================

pub struct ConfigReader;

impl ConfigReader {
    /// Load user configuration file
    pub fn load_user_config(path: &PathBuf) -> Result<UserConfig, String> {
        // Check if file exists
        if !path.exists() {
            return Err(format!(
                "Configuration file not found: {}\nPlease create the configuration file or run 'kagent.sh config' to configure.",
                path.display()
            ));
        }

        // Read file content
        let content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read configuration file: {}", e))?;

        // Parse configuration
        UserConfig::decode(&content)
    }

    /// Save user configuration file
    pub fn save_user_config(path: &PathBuf, config: &UserConfig) -> Result<(), String> {
        let content = config.encode();

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config directory: {}", e))?;
        }

        // Write file
        fs::write(path, content)
            .map_err(|e| format!("Failed to write configuration file: {}", e))?;

        info!("Configuration file saved to: {}", path.display());
        Ok(())
    }

    /// Load runtime data file
    #[allow(dead_code)]
    pub fn load_runtime_data(path: &PathBuf) -> Result<RuntimeData, String> {
        // If file doesn't exist, return default
        if !path.exists() {
            return Ok(RuntimeData::default());
        }

        // Read file content
        let content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read runtime data file: {}", e))?;

        // Parse runtime data
        RuntimeData::decode(&content)
    }

    /// Save runtime data file
    #[allow(dead_code)]
    pub fn save_runtime_data(path: &PathBuf, data: &RuntimeData) -> Result<(), String> {
        let content = data.encode();

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create runtime data directory: {}", e))?;
        }

        // Write file
        fs::write(path, content)
            .map_err(|e| format!("Failed to write runtime data file: {}", e))?;

        Ok(())
    }
}

// ==================== Helper Functions ====================

fn is_root_user() -> bool {
    #[cfg(unix)]
    {
        // On Unix systems, use geteuid() system call to get effective user ID
        unsafe { libc::geteuid() == 0 }
    }

    #[cfg(windows)]
    {
        // On Windows, check environment variables or use Windows API
        // For simplicity, we check if running as SYSTEM or Administrator
        env::var("USERNAME")
            .map(|u| u.eq_ignore_ascii_case("SYSTEM") || u.eq_ignore_ascii_case("Administrator"))
            .unwrap_or(false)
    }
}

fn get_default_terminal_entry() -> String {
    if cfg!(windows) {
        "cmd.exe".to_string()
    } else if fs::exists("/bin/bash").unwrap_or(false) {
        "bash".to_string()
    } else {
        "sh".to_string()
    }
}

fn parse_bool(value: &str, key: &str) -> Result<bool, String> {
    match value.to_lowercase().as_str() {
        "true" | "1" | "yes" => Ok(true),
        "false" | "0" | "no" => Ok(false),
        _ => Err(format!("Invalid boolean value for {}: {}", key, value)),
    }
}

fn parse_u8(value: &str, key: &str) -> Result<u8, String> {
    value
        .parse::<u8>()
        .map_err(|_| format!("Invalid u8 value for {}: {}", key, value))
}

fn parse_u32(value: &str, key: &str) -> Result<u32, String> {
    value
        .parse::<u32>()
        .map_err(|_| format!("Invalid u32 value for {}: {}", key, value))
}

fn parse_u64(value: &str, key: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .map_err(|_| format!("Invalid u64 value for {}: {}", key, value))
}

fn parse_f64(value: &str, key: &str) -> Result<f64, String> {
    value
        .parse::<f64>()
        .map_err(|_| format!("Invalid f64 value for {}: {}", key, value))
}
