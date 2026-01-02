use crate::command_parser::NetworkConfig;
use crate::get_info::network::filter_network;
use log::{error, info, warn};
use miniserde::{Deserialize, Serialize};
use std::cmp::PartialEq;
use std::fs;
use std::io::SeekFrom;
use std::str::FromStr;
use std::time::Duration;
use sysinfo::Networks;
use time::OffsetDateTime;
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

/// Get current month (1-12)
fn get_current_month() -> u8 {
    match OffsetDateTime::now_local() {
        Ok(now) => now.month() as u8,
        Err(_) => {
            // Fallback to UTC if local time fails
            OffsetDateTime::now_utc().month() as u8
        }
    }
}

/// Get current day of month (1-31)
fn get_current_day() -> u8 {
    match OffsetDateTime::now_local() {
        Ok(now) => now.day(),
        Err(_) => {
            // Fallback to UTC if local time fails
            OffsetDateTime::now_utc().day()
        }
    }
}

/// Get the number of days in the current month
fn get_days_in_current_month() -> u8 {
    let now = match OffsetDateTime::now_local() {
        Ok(now) => now,
        Err(_) => OffsetDateTime::now_utc(),
    };

    let year = now.year();
    let month = now.month();

    // Calculate days in month
    match month {
        time::Month::January | time::Month::March | time::Month::May
        | time::Month::July | time::Month::August | time::Month::October
        | time::Month::December => 31,
        time::Month::April | time::Month::June
        | time::Month::September | time::Month::November => 30,
        time::Month::February => {
            // Check for leap year
            if (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0) {
                29
            } else {
                28
            }
        }
    }
}

/// Get the effective reset day for the current month
/// If reset_day is greater than the number of days in the month, use the last day of the month
fn get_effective_reset_day(reset_day: u8) -> u8 {
    let days_in_month = get_days_in_current_month();
    reset_day.min(days_in_month)
}

/// Check if traffic should be reset based on current date and reset configuration
fn should_reset_traffic(last_reset_month: u8, reset_day: u8) -> bool {
    let current_month = get_current_month();
    let current_day = get_current_day();
    let effective_reset_day = get_effective_reset_day(reset_day);

    // Reset if we're in a new month and have reached or passed the effective reset day
    current_month != last_reset_month && current_day >= effective_reset_day
}

#[derive(Serialize, Deserialize, PartialEq)]
struct NetworkInfo {
    config: NetworkConfig,
    boot_id: String,
    source_tx: u64,
    source_rx: u64,
    latest_tx: u64,
    latest_rx: u64,
    last_reset_month: u8,
}

impl NetworkInfo {
    pub fn encode(&self) -> String {
        let mut output = String::new();

        macro_rules! append_line {
            ($key:expr, $value:expr) => {
                output.push_str(&format!("{}={}\n", $key, $value));
            };
        }

        append_line!(
            "disable_network_statistics",
            self.config.disable_network_statistics
        );
        append_line!("network_interval", self.config.network_interval);
        append_line!("reset_day", self.config.reset_day);
        append_line!("calibration_tx", self.config.calibration_tx);
        append_line!("calibration_rx", self.config.calibration_rx);
        append_line!("network_save_path", self.config.network_save_path);

        append_line!("boot_id", self.boot_id);
        append_line!("source_tx", self.source_tx);
        append_line!("source_rx", self.source_rx);
        append_line!("latest_tx", self.latest_tx);
        append_line!("latest_rx", self.latest_rx);
        append_line!("last_reset_month", self.last_reset_month);

        output
    }

    /// Decoder: Parses NetworkInfo from a String
    pub fn decode(input: &str) -> Result<Self, String> {
        let mut disable_network_statistics = None;
        let mut network_interval = None;
        let mut reset_day = None;
        let mut calibration_tx = None;
        let mut calibration_rx = None;
        let mut network_save_path = None;
        let mut boot_id = None;
        let mut source_tx = None;
        let mut source_rx = None;
        let mut latest_tx = None;
        let mut latest_rx = None;
        let mut last_reset_month = None;

        for (line_num, line) in input.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let (key, value) = line.split_once('=').ok_or_else(|| {
                format!("Line {}: Format error (expected key=value)", line_num + 1)
            })?;

            let key = key.trim();
            let value = value.trim();

            let parse_err = |type_name: &str| {
                format!(
                    "Line {}: Invalid {} for key '{}'",
                    line_num + 1,
                    type_name,
                    key
                )
            };

            match key {
                "disable_network_statistics" => {
                    disable_network_statistics =
                        Some(FromStr::from_str(value).map_err(|_| parse_err("bool"))?)
                }
                "network_interval" => {
                    network_interval = Some(value.parse().map_err(|_| parse_err("u32"))?)
                }
                "reset_day" => {
                    reset_day = Some(value.parse().map_err(|_| parse_err("u8"))?)
                }
                "calibration_tx" => {
                    calibration_tx = Some(value.parse().map_err(|_| parse_err("u64"))?)
                }
                "calibration_rx" => {
                    calibration_rx = Some(value.parse().map_err(|_| parse_err("u64"))?)
                }
                "network_save_path" => network_save_path = Some(value.to_string()),
                "boot_id" => boot_id = Some(value.to_string()),
                "source_tx" => source_tx = Some(value.parse().map_err(|_| parse_err("u64"))?),
                "source_rx" => source_rx = Some(value.parse().map_err(|_| parse_err("u64"))?),
                "latest_tx" => latest_tx = Some(value.parse().map_err(|_| parse_err("u64"))?),
                "latest_rx" => latest_rx = Some(value.parse().map_err(|_| parse_err("u64"))?),
                "last_reset_month" => last_reset_month = Some(value.parse().map_err(|_| parse_err("u8"))?),
                _ => {}
            }
        }

        // Assemble the struct, check if required fields exist
        Ok(NetworkInfo {
            config: NetworkConfig {
                disable_network_statistics: disable_network_statistics
                    .ok_or("Missing field: disable_network_statistics")?,
                network_interval: network_interval.ok_or("Missing field: network_interval")?,
                reset_day: reset_day.ok_or("Missing field: reset_day")?,
                calibration_tx: calibration_tx.ok_or("Missing field: calibration_tx")?,
                calibration_rx: calibration_rx.ok_or("Missing field: calibration_rx")?,
                network_save_path: network_save_path.ok_or("Missing field: network_save_path")?,
            },
            boot_id: boot_id.ok_or("Missing field: boot_id")?,
            source_tx: source_tx.ok_or("Missing field: source_tx")?,
            source_rx: source_rx.ok_or("Missing field: source_rx")?,
            latest_tx: latest_tx.ok_or("Missing field: latest_tx")?,
            latest_rx: latest_rx.ok_or("Missing field: latest_rx")?,
            last_reset_month: last_reset_month.ok_or("Missing field: last_reset_month")?,
        })
    }
}

async fn get_or_init_latest_network_info(
    network_config: &NetworkConfig,
) -> Result<(File, NetworkInfo), String> {
    let mut file = match OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&network_config.network_save_path)
        .await
    {
        Ok(file) => file,
        Err(e) => {
            return Err(format!("Failed to open network traffic info file: {e}"));
        }
    };

    let new_boot_id = if cfg!(target_os = "linux") {
        match fs::read_to_string("/proc/sys/kernel/random/boot_id") {
            Ok(content) => content.trim().to_string(),
            Err(e) => {
                warn!("Failed to read boot_id: {}", e);
                String::new()
            }
        }
    } else {
        String::new()
    };

    let mut raw_data = String::new();
    file.read_to_string(&mut raw_data)
        .await
        .map_err(|e| format!("Failed to read network traffic info file: {e}"))?;

    let raw_network_info = if raw_data.is_empty() {
        let network_info = NetworkInfo {
            config: network_config.clone(),
            boot_id: new_boot_id.clone(),
            source_tx: 0,
            source_rx: 0,
            latest_tx: 0,
            latest_rx: 0,
            last_reset_month: get_current_month(),
        };
        rewrite_network_info_file(&mut file, network_info.encode())
            .await
            .map_err(|e| format!("Failed to write network traffic info file: {e}"))?;
        info!(
            "Network traffic info file is empty, possibly first run or save path changed, created new file"
        );
        network_info
    } else {
        let raw_network_info = NetworkInfo::decode(&raw_data);

        if let Err(e) = &raw_network_info {
            warn!(
                "Failed to parse network traffic info file: {}. Will recreate the file in 3 seconds.",
                e
            );
            tokio::time::sleep(Duration::from_secs(3)).await;

            drop(file);
            if let Err(e) = tokio::fs::remove_file(&network_config.network_save_path).await {
                return Err(format!(
                    "Failed to remove corrupted network traffic info file: {e}"
                ));
            }

            file = reopen_network_file(&network_config.network_save_path).await?;

            let network_info = NetworkInfo {
                config: network_config.clone(),
                boot_id: new_boot_id.clone(),
                source_tx: 0,
                source_rx: 0,
                latest_tx: 0,
                latest_rx: 0,
                last_reset_month: get_current_month(),
            };
            rewrite_network_info_file(&mut file, network_info.encode())
                .await
                .map_err(|e| format!("Failed to write network traffic info file: {e}"))?;
            info!("Recreated network traffic info file");
            network_info
        } else {
            let raw_network_info = raw_network_info?;

            if &raw_network_info.config != network_config {
                info!("Network configuration changed, applying new configuration while preserving traffic data");
                let network_info = NetworkInfo {
                    config: network_config.clone(),
                    boot_id: raw_network_info.boot_id,
                    source_tx: raw_network_info.source_tx,
                    source_rx: raw_network_info.source_rx,
                    latest_tx: raw_network_info.latest_tx,
                    latest_rx: raw_network_info.latest_rx,
                    last_reset_month: raw_network_info.last_reset_month,
                };
                rewrite_network_info_file(&mut file, network_info.encode())
                    .await
                    .map_err(|e| format!("Failed to write network traffic info file: {e}"))?;
                info!("Network configuration updated");
                network_info
            } else {
                raw_network_info
            }
        }
    };

    // Handle system reboot: merge latest traffic into source
    let new_network_info = if cfg!(target_os = "linux") && !new_boot_id.is_empty() {
        if raw_network_info.boot_id != new_boot_id {
            info!("System reboot detected, merging traffic data");
            let network_info = NetworkInfo {
                config: raw_network_info.config,
                boot_id: new_boot_id,
                source_tx: raw_network_info.source_tx + raw_network_info.latest_tx,
                source_rx: raw_network_info.source_rx + raw_network_info.latest_rx,
                latest_tx: 0,
                latest_rx: 0,
                last_reset_month: raw_network_info.last_reset_month,
            };
            rewrite_network_info_file(&mut file, network_info.encode())
                .await
                .map_err(|e| format!("Failed to write network traffic info file: {e}"))?;
            network_info
        } else {
            raw_network_info
        }
    } else if cfg!(target_os = "windows") {
        // Windows: always merge on startup as we can't reliably detect reboots
        let network_info = NetworkInfo {
            config: raw_network_info.config,
            boot_id: new_boot_id,
            source_tx: raw_network_info.source_tx + raw_network_info.latest_tx,
            source_rx: raw_network_info.source_rx + raw_network_info.latest_rx,
            latest_tx: 0,
            latest_rx: 0,
            last_reset_month: raw_network_info.last_reset_month,
        };
        rewrite_network_info_file(&mut file, network_info.encode())
            .await
            .map_err(|e| format!("Failed to write network traffic info file: {e}"))?;
        network_info
    } else {
        raw_network_info
    };

    Ok((file, new_network_info))
}

pub async fn network_saver(
    tx: tokio::sync::mpsc::Sender<(u64, u64)>,
    network_config: &NetworkConfig,
) {
    if network_config.disable_network_statistics {
        return;
    }

    let (mut file, mut network_info) = match get_or_init_latest_network_info(&network_config).await
    {
        Ok(n) => n,
        Err(e) => {
            warn!("An error occurred while getting or initing network traffic info: {e}.");
            warn!(
                "This will fallback to statistics only showing network interface traffic since the current startup, equivalent to `--disable-network-statistics`."
            );
            return;
        }
    };

    let mut networks = Networks::new_with_refreshed_list();
    let mut save_counter = 0u32; // Counter for periodic disk writes

    loop {
        networks.refresh(true);
        let (_, _, total_up, total_down) = filter_network(&networks);

        // Check if we need to reset traffic based on monthly schedule
        if should_reset_traffic(network_info.last_reset_month, network_info.config.reset_day) {
            let current_month = get_current_month();
            let effective_reset_day = get_effective_reset_day(network_info.config.reset_day);
            info!(
                "Monthly traffic reset triggered (configured day: {}, effective day: {}, current month: {})",
                network_info.config.reset_day, effective_reset_day, current_month
            );

            network_info = NetworkInfo {
                config: network_info.config.clone(),
                boot_id: network_info.boot_id.clone(),
                source_tx: total_up,  // Set current system traffic as new baseline
                source_rx: total_down,
                latest_tx: 0,         // Reset latest to 0
                latest_rx: 0,
                last_reset_month: current_month,
            };

            // Immediately save the reset state
            if let Err(e) = rewrite_network_info_file(&mut file, network_info.encode()).await {
                error!("Failed to write network traffic info file after reset: {e}");
            } else {
                info!("Traffic statistics reset completed");
            }
        } else {
            // Normal update: just update latest traffic
            network_info.latest_tx = total_up.saturating_sub(network_info.source_tx);
            network_info.latest_rx = total_down.saturating_sub(network_info.source_rx);
        }

        save_counter += 1;

        // Periodically save to disk (every 10 intervals by default)
        if save_counter >= 10 {
            if let Err(e) = rewrite_network_info_file(&mut file, network_info.encode()).await {
                error!("Failed to write network traffic info file: {e}");
            }
            save_counter = 0;
        }

        // Send total traffic including calibration values to the main loop
        let total_tx = network_info.latest_tx + network_info.config.calibration_tx;
        let total_rx = network_info.latest_rx + network_info.config.calibration_rx;

        if let Err(e) = tx.send((total_tx, total_rx)).await {
            error!("Failed to send traffic data: {e}");
        }

        tokio::time::sleep(Duration::from_secs(
            network_info.config.network_interval as u64,
        ))
        .await;

        // Check for configuration changes (hot reload)
        // Re-read the config file to detect external changes
        let mut config_file = match File::open(&network_info.config.network_save_path).await {
            Ok(f) => f,
            Err(_) => continue, // If can't open, skip config reload check
        };

        let mut config_data = String::new();
        if config_file.read_to_string(&mut config_data).await.is_ok() {
            if let Ok(updated_info) = NetworkInfo::decode(&config_data) {
                // Check if calibration values or other settings changed
                if updated_info.config != network_info.config {
                    info!("Configuration file changed detected, reloading configuration");

                    // Preserve traffic data but apply new config
                    network_info.config = updated_info.config.clone();

                    // Save with updated config
                    if let Err(e) = rewrite_network_info_file(&mut file, network_info.encode()).await {
                        error!("Failed to save updated configuration: {e}");
                    } else {
                        info!("Configuration reloaded successfully");
                    }
                }
            }
        }
    }
}

async fn rewrite_network_info_file(
    file: &mut File,
    string: String,
) -> Result<(), Box<dyn std::error::Error>> {
    file.set_len(0).await?;
    file.seek(SeekFrom::Start(0)).await?;
    file.write_all(string.as_bytes()).await?;
    Ok(())
}

async fn reopen_network_file(path: &str) -> Result<File, String> {
    match OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .await
    {
        Ok(file) => Ok(file),
        Err(e) => Err(format!("Failed to reopen network traffic info file: {e}")),
    }
}
