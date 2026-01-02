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

#[derive(Serialize, Deserialize, PartialEq)]
struct NetworkInfo {
    config: NetworkConfig,
    boot_id: String,
    source_tx: u64,
    source_rx: u64,
    latest_tx: u64,
    latest_rx: u64,
    last_reset_month: String,  // Format: "YYYY-MM" to track last reset
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
        append_line!("network_reset_day", self.config.network_reset_day);
        append_line!("network_interval", self.config.network_interval);
        append_line!(
            "network_interval_number",
            self.config.network_interval_number
        );
        append_line!("network_calibration_tx", self.config.network_calibration_tx);
        append_line!("network_calibration_rx", self.config.network_calibration_rx);
        append_line!("network_save_path", self.config.network_save_path);
        if let Some(config_path) = &self.config.config_path {
            append_line!("config_path", config_path);
        }

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
        let mut network_reset_day = None;
        let mut network_interval = None;
        let mut network_interval_number = None;
        let mut network_calibration_tx = None;
        let mut network_calibration_rx = None;
        let mut network_save_path = None;
        let mut config_path = None;
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
                "network_reset_day" => {
                    network_reset_day = Some(value.parse().map_err(|_| parse_err("u32"))?)
                }
                "network_interval" => {
                    network_interval = Some(value.parse().map_err(|_| parse_err("u32"))?)
                }
                "network_interval_number" => {
                    network_interval_number = Some(value.parse().map_err(|_| parse_err("u32"))?)
                }
                "network_calibration_tx" => {
                    network_calibration_tx = Some(value.parse().map_err(|_| parse_err("i64"))?)
                }
                "network_calibration_rx" => {
                    network_calibration_rx = Some(value.parse().map_err(|_| parse_err("i64"))?)
                }
                "network_save_path" => network_save_path = Some(value.to_string()),
                "config_path" => config_path = Some(value.to_string()),
                "boot_id" => boot_id = Some(value.to_string()),
                "source_tx" => source_tx = Some(value.parse().map_err(|_| parse_err("u64"))?),
                "source_rx" => source_rx = Some(value.parse().map_err(|_| parse_err("u64"))?),
                "latest_tx" => latest_tx = Some(value.parse().map_err(|_| parse_err("u64"))?),
                "latest_rx" => latest_rx = Some(value.parse().map_err(|_| parse_err("u64"))?),
                "last_reset_month" => last_reset_month = Some(value.to_string()),
                _ => {}
            }
        }

        // Assemble the struct, check if required fields exist
        Ok(NetworkInfo {
            config: NetworkConfig {
                disable_network_statistics: disable_network_statistics
                    .ok_or("Missing field: disable_network_statistics")?,
                network_reset_day: network_reset_day.ok_or("Missing field: network_reset_day")?,
                network_interval: network_interval.ok_or("Missing field: network_interval")?,
                network_interval_number: network_interval_number
                    .ok_or("Missing field: network_interval_number")?,
                network_calibration_tx: network_calibration_tx
                    .ok_or("Missing field: network_calibration_tx")?,
                network_calibration_rx: network_calibration_rx
                    .ok_or("Missing field: network_calibration_rx")?,
                network_save_path: network_save_path.ok_or("Missing field: network_save_path")?,
                config_path,
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

    // Get current month in YYYY-MM format
    let now = OffsetDateTime::now_utc();
    let current_month = format!("{}-{:02}", now.year(), now.month() as u8);

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
            last_reset_month: current_month,
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
                last_reset_month: current_month,
            };
            rewrite_network_info_file(&mut file, network_info.encode())
                .await
                .map_err(|e| format!("Failed to write network traffic info file: {e}"))?;
            info!("Recreated network traffic info file");
            network_info
        } else {
            raw_network_info?
        }
    };

    // Check if configuration changed or we need to reset due to boot_id change
    let config_changed = &raw_network_info.config != network_config;
    let boot_changed = cfg!(target_os = "linux") 
        && !new_boot_id.is_empty() 
        && raw_network_info.boot_id != new_boot_id;

    if config_changed {
        warn!(
            "Network traffic configuration changed. Resetting statistics in 3 seconds. Press Ctrl+C to stop."
        );
        tokio::time::sleep(Duration::from_secs(3)).await;
        warn!("Clearing network traffic info");
        
        let network_info = NetworkInfo {
            config: network_config.clone(),
            boot_id: new_boot_id.clone(),
            source_tx: 0,
            source_rx: 0,
            latest_tx: 0,
            latest_rx: 0,
            last_reset_month: current_month,
        };
        rewrite_network_info_file(&mut file, network_info.encode())
            .await
            .map_err(|e| format!("Failed to write network traffic info file: {e}"))?;
        info!("Network traffic info cleared due to configuration change");
        return Ok((file, network_info));
    }

    if boot_changed {
        info!("System reboot detected, accumulating previous session traffic");
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
        return Ok((file, network_info));
    }

    Ok((file, raw_network_info))
}

/// Helper function to check if we should reset traffic based on reset day
fn should_reset_traffic(last_reset_month: &str, reset_day: u32) -> bool {
    let now = OffsetDateTime::now_utc();
    let current_month = format!("{}-{:02}", now.year(), now.month() as u8);
    let current_day = now.day();
    
    // If we're in a new month and past the reset day, or it's the reset day and we haven't reset this month
    if last_reset_month != current_month && current_day >= reset_day {
        return true;
    }
    
    false
}

/// Load configuration from file if config_path is specified
async fn load_config_from_file(config_path: &str) -> Result<NetworkConfig, String> {
    let content = tokio::fs::read_to_string(config_path)
        .await
        .map_err(|e| format!("Failed to read config file: {}", e))?;
    
    // Parse the config file (same format as network info file)
    let mut disable_network_statistics = None;
    let mut network_reset_day = None;
    let mut network_interval = None;
    let mut network_interval_number = None;
    let mut network_calibration_tx = None;
    let mut network_calibration_rx = None;
    
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim();
            
            match key {
                "disable_network_statistics" => {
                    disable_network_statistics = value.parse().ok();
                }
                "network_reset_day" => {
                    network_reset_day = value.parse().ok();
                }
                "network_interval" => {
                    network_interval = value.parse().ok();
                }
                "network_interval_number" => {
                    network_interval_number = value.parse().ok();
                }
                "network_calibration_tx" => {
                    network_calibration_tx = value.parse().ok();
                }
                "network_calibration_rx" => {
                    network_calibration_rx = value.parse().ok();
                }
                _ => {}
            }
        }
    }
    
    // Return error if any required field is missing
    Ok(NetworkConfig {
        disable_network_statistics: disable_network_statistics
            .ok_or("Missing disable_network_statistics in config")?,
        network_reset_day: network_reset_day.ok_or("Missing network_reset_day in config")?,
        network_interval: network_interval.ok_or("Missing network_interval in config")?,
        network_interval_number: network_interval_number
            .ok_or("Missing network_interval_number in config")?,
        network_calibration_tx: network_calibration_tx
            .ok_or("Missing network_calibration_tx in config")?,
        network_calibration_rx: network_calibration_rx
            .ok_or("Missing network_calibration_rx in config")?,
        network_save_path: String::new(), // Will be filled from current config
        config_path: None,
    })
}

pub async fn network_saver(
    tx: tokio::sync::mpsc::Sender<(u64, u64)>,
    network_config: &NetworkConfig,
) {
    if network_config.disable_network_statistics {
        return;
    }

    let mut current_config = network_config.clone();
    let mut config_file_modified_time = if let Some(config_path) = &network_config.config_path {
        tokio::fs::metadata(config_path)
            .await
            .ok()
            .and_then(|m| m.modified().ok())
    } else {
        None
    };

    loop {
        let (mut file, mut network_info) = match get_or_init_latest_network_info(&current_config)
            .await
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
        let mut memory_update_count = 0;

        loop {
            // Check for config file changes (hot reload)
            if let Some(config_path) = &current_config.config_path {
                if let Ok(metadata) = tokio::fs::metadata(config_path).await {
                    if let Ok(modified) = metadata.modified() {
                        if config_file_modified_time.map_or(true, |last| modified > last) {
                            info!("Config file changed, reloading configuration...");
                            match load_config_from_file(config_path).await {
                                Ok(mut new_config) => {
                                    new_config.network_save_path = current_config.network_save_path.clone();
                                    new_config.config_path = current_config.config_path.clone();
                                    
                                    // Update calibration values in current network_info
                                    network_info.config = new_config.clone();
                                    current_config = new_config;
                                    config_file_modified_time = Some(modified);
                                    
                                    info!("Configuration reloaded successfully");
                                    info!("New calibration TX: {}, RX: {}", 
                                          current_config.network_calibration_tx,
                                          current_config.network_calibration_rx);
                                }
                                Err(e) => {
                                    warn!("Failed to reload config: {}", e);
                                }
                            }
                        }
                    }
                }
            }

            networks.refresh(true);
            let (_, _, total_up, total_down) = filter_network(&networks);

            // Check if we should reset traffic based on the reset day
            if should_reset_traffic(&network_info.last_reset_month, current_config.network_reset_day) {
                info!("Monthly reset day reached, resetting traffic statistics");
                let now = OffsetDateTime::now_utc();
                let current_month = format!("{}-{:02}", now.year(), now.month() as u8);
                
                network_info = NetworkInfo {
                    config: current_config.clone(),
                    boot_id: network_info.boot_id.clone(),
                    source_tx: 0,
                    source_rx: 0,
                    latest_tx: 0,
                    latest_rx: 0,
                    last_reset_month: current_month,
                };
                
                if let Err(e) = rewrite_network_info_file(&mut file, network_info.encode()).await {
                    error!("Failed to write network traffic info file after reset: {e}");
                }
                
                info!("Traffic statistics have been reset for the new billing cycle");
            }

            network_info.latest_tx = total_up;
            network_info.latest_rx = total_down;
            memory_update_count += 1;

            // Save to disk periodically
            if memory_update_count >= current_config.network_interval_number {
                if let Err(e) =
                    rewrite_network_info_file(&mut file, network_info.encode()).await
                {
                    error!("Failed to write network traffic info file: {e}");
                    continue;
                }
                memory_update_count = 0;
            }

            // Apply calibration and send total traffic (including calibration offset)
            let calibrated_tx = (network_info.latest_tx + network_info.source_tx) as i64 
                + current_config.network_calibration_tx;
            let calibrated_rx = (network_info.latest_rx + network_info.source_rx) as i64 
                + current_config.network_calibration_rx;
            
            // Ensure non-negative values
            let final_tx = calibrated_tx.max(0) as u64;
            let final_rx = calibrated_rx.max(0) as u64;

            if let Err(e) = tx.send((final_tx, final_rx)).await {
                error!("Failed to send traffic data: {e}");
                continue;
            }

            tokio::time::sleep(Duration::from_secs(
                current_config.network_interval as u64,
            ))
            .await;
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
