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
    counter: u32,  // Kept for backward compatibility, no longer used
    last_reset_date: String,  // Format: "YYYY-MM-DD"
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
        append_line!("network_duration", self.config.network_duration);
        append_line!("network_interval", self.config.network_interval);
        append_line!(
            "network_interval_number",
            self.config.network_interval_number
        );
        append_line!("network_save_path", self.config.network_save_path);

        append_line!("boot_id", self.boot_id);
        append_line!("source_tx", self.source_tx);
        append_line!("source_rx", self.source_rx);
        append_line!("latest_tx", self.latest_tx);
        append_line!("latest_rx", self.latest_rx);
        append_line!("counter", self.counter);
        append_line!("last_reset_date", self.last_reset_date);

        output
    }

    /// Decoder: Parses NetworkInfo from a String
    pub fn decode(input: &str) -> Result<Self, String> {
        let mut disable_network_statistics = None;
        let mut network_duration = None;
        let mut network_interval = None;
        let mut network_interval_number = None;
        let mut network_save_path = None;
        let mut boot_id = None;
        let mut source_tx = None;
        let mut source_rx = None;
        let mut latest_tx = None;
        let mut latest_rx = None;
        let mut counter = None;
        let mut network_reset_day = None;
        let mut last_reset_date = None;

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
                "network_duration" => {
                    network_duration = Some(value.parse().map_err(|_| parse_err("u32"))?)
                }
                "network_interval" => {
                    network_interval = Some(value.parse().map_err(|_| parse_err("u32"))?)
                }
                "network_interval_number" => {
                    network_interval_number = Some(value.parse().map_err(|_| parse_err("u32"))?)
                }
                "network_save_path" => network_save_path = Some(value.to_string()),
                "boot_id" => boot_id = Some(value.to_string()),
                "source_tx" => source_tx = Some(value.parse().map_err(|_| parse_err("u64"))?),
                "source_rx" => source_rx = Some(value.parse().map_err(|_| parse_err("u64"))?),
                "latest_tx" => latest_tx = Some(value.parse().map_err(|_| parse_err("u64"))?),
                "latest_rx" => latest_rx = Some(value.parse().map_err(|_| parse_err("u64"))?),
                "counter" => counter = Some(value.parse().map_err(|_| parse_err("u32"))?),
                "network_reset_day" => {
                    network_reset_day = Some(value.parse().map_err(|_| parse_err("u8"))?)
                }
                "last_reset_date" => last_reset_date = Some(value.to_string()),
                _ => {}
            }
        }

        // Assemble the struct, check if required fields exist
        Ok(NetworkInfo {
            config: NetworkConfig {
                disable_network_statistics: disable_network_statistics
                    .ok_or("Missing field: disable_network_statistics")?,
                network_duration: network_duration.ok_or("Missing field: network_duration")?,
                network_interval: network_interval.ok_or("Missing field: network_interval")?,
                network_interval_number: network_interval_number
                    .ok_or("Missing field: network_interval_number")?,
                network_save_path: network_save_path.ok_or("Missing field: network_save_path")?,
                network_reset_day: network_reset_day.unwrap_or(1),  // Default to 1st of month
            },
            boot_id: boot_id.ok_or("Missing field: boot_id")?,
            source_tx: source_tx.ok_or("Missing field: source_tx")?,
            source_rx: source_rx.ok_or("Missing field: source_rx")?,
            latest_tx: latest_tx.ok_or("Missing field: latest_tx")?,
            latest_rx: latest_rx.ok_or("Missing field: latest_rx")?,
            counter: counter.unwrap_or(0),  // Backward compatibility, no longer used
            last_reset_date: last_reset_date.unwrap_or_else(|| {
                // Backward compatibility: use current date for old config files
                use time::OffsetDateTime;
                let now = OffsetDateTime::now_local()
                    .unwrap_or_else(|_| OffsetDateTime::now_utc());
                format!("{:04}-{:02}-{:02}", now.year(), now.month() as u8, now.day())
            }),
        })
    }
}

/// Get current date as string (format: YYYY-MM-DD)
fn get_current_date_string() -> String {
    use time::OffsetDateTime;
    let now = OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc());
    format!("{:04}-{:02}-{:02}", now.year(), now.month() as u8, now.day())
}

/// Get number of days in a given month
fn days_in_month(year: i32, month: u8) -> u8 {
    use time::Month;
    match Month::try_from(month) {
        Ok(month_enum) => time::util::days_in_year_month(year, month_enum),
        Err(_) => 31,  // Fallback, should never happen with valid month
    }
}

/// Get actual reset day for the month (handles month-end boundaries)
fn get_actual_reset_day(year: i32, month: u8, desired_day: u8) -> u8 {
    let max_day = days_in_month(year, month);
    desired_day.min(max_day)
}

/// Check if monthly reset should be triggered
fn should_reset_monthly(last_reset_date: &str, reset_day: u8) -> Result<bool, String> {
    use time::OffsetDateTime;

    // Parse last reset date
    let parts: Vec<&str> = last_reset_date.split('-').collect();
    if parts.len() != 3 {
        return Err(format!("Invalid date format: {}", last_reset_date));
    }

    let last_year: i32 = parts[0]
        .parse()
        .map_err(|_| format!("Invalid year: {}", parts[0]))?;
    let last_month: u8 = parts[1]
        .parse()
        .map_err(|_| format!("Invalid month: {}", parts[1]))?;
    let last_day: u8 = parts[2]
        .parse()
        .map_err(|_| format!("Invalid day: {}", parts[2]))?;

    // Get current date
    let now = OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc());
    let current_year = now.year();
    let current_month = now.month() as u8;
    let current_day = now.day();

    // Calculate actual reset day for current month
    let actual_reset_day = get_actual_reset_day(current_year, current_month, reset_day);

    // Case 1: Year changed
    if current_year > last_year {
        return Ok(true);
    }

    // Case 2: Month changed (same year)
    if current_year == last_year && current_month > last_month {
        return Ok(true);
    }

    // Case 3: Same year and month, current day >= reset day AND last reset was before reset day
    if current_year == last_year && current_month == last_month {
        if current_day >= actual_reset_day && last_day < actual_reset_day {
            return Ok(true);
        }
    }

    // Case 4: Clock moved backwards
    if current_year == last_year && current_month == last_month && current_day < last_day {
        warn!("System clock moved backwards, resetting network statistics");
        return Ok(true);
    }

    Ok(false)
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
            counter: 0,  // No longer used for reset logic
            last_reset_date: get_current_date_string(),
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
                counter: 0,  // No longer used for reset logic
                last_reset_date: get_current_date_string(),
            };
            rewrite_network_info_file(&mut file, network_info.encode())
                .await
                .map_err(|e| format!("Failed to write network traffic info file: {e}"))?;
            info!("Recreated network traffic info file");
            network_info
        } else {
            let raw_network_info = raw_network_info?;

            if &raw_network_info.config != network_config {
                warn!(
                    "Network traffic info file configuration does not match, overwriting file and resetting statistics in 3 seconds. Press Ctrl+C to stop."
                );
                tokio::time::sleep(Duration::from_secs(3)).await;
                warn!("Starting to clear network traffic info");
                let network_info = NetworkInfo {
                    config: network_config.clone(),
                    boot_id: new_boot_id.clone(),
                    source_tx: 0,
                    source_rx: 0,
                    latest_tx: 0,
                    latest_rx: 0,
                    counter: 0,  // No longer used for reset logic
                    last_reset_date: get_current_date_string(),
                };
                rewrite_network_info_file(&mut file, network_info.encode())
                    .await
                    .map_err(|e| format!("Failed to write network traffic info file: {e}"))?;
                info!("Network traffic info cleared");
                network_info
            } else {
                raw_network_info
            }
        }
    };

    // Check if monthly reset should be triggered
    if should_reset_monthly(&raw_network_info.last_reset_date, raw_network_info.config.network_reset_day)
        .unwrap_or(false)
    {
        info!(
            "Monthly reset day {} reached, resetting network statistics",
            raw_network_info.config.network_reset_day
        );
        let network_info = NetworkInfo {
            config: raw_network_info.config,
            boot_id: new_boot_id.clone(),
            source_tx: 0,  // Complete reset
            source_rx: 0,
            latest_tx: 0,
            latest_rx: 0,
            counter: 0,
            last_reset_date: get_current_date_string(),
        };
        rewrite_network_info_file(&mut file, network_info.encode())
            .await
            .map_err(|e| format!("Failed to write network traffic info file: {e}"))?;
        return Ok((file, network_info));
    }

    // Handle system reboot (boot_id changed)
    let new_network_info = if cfg!(target_os = "linux") && !new_boot_id.is_empty() {
        if raw_network_info.boot_id != new_boot_id {
            // System rebooted - accumulate traffic to source
            NetworkInfo {
                config: raw_network_info.config,
                boot_id: new_boot_id,
                source_tx: raw_network_info.source_tx + raw_network_info.latest_tx,
                source_rx: raw_network_info.source_rx + raw_network_info.latest_rx,
                latest_tx: 0,
                latest_rx: 0,
                counter: 0,  // No longer used
                last_reset_date: raw_network_info.last_reset_date,  // Keep reset date unchanged
            }
        } else {
            // No reboot detected - keep data unchanged
            raw_network_info
        }
    } else {
        // Non-Linux or no boot_id - assume reboot
        NetworkInfo {
            config: raw_network_info.config,
            boot_id: new_boot_id,
            source_tx: raw_network_info.source_tx + raw_network_info.latest_tx,
            source_rx: raw_network_info.source_rx + raw_network_info.latest_rx,
            latest_tx: 0,
            latest_rx: 0,
            counter: 0,  // No longer used
            last_reset_date: raw_network_info.last_reset_date,  // Keep reset date unchanged
        }
    };

    rewrite_network_info_file(&mut file, new_network_info.encode())
        .await
        .map_err(|e| format!("Failed to write network traffic info file: {e}"))?;

    Ok((file, new_network_info))
}

pub async fn network_saver(
    tx: tokio::sync::mpsc::Sender<(u64, u64)>,
    network_config: &NetworkConfig,
) {
    if network_config.disable_network_statistics {
        return;
    }

    loop {
        let (mut file, mut network_info) = match get_or_init_latest_network_info(&network_config)
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

        // Add a counter to accumulate memory update times
        let mut memory_update_count = 0;

        loop {
            networks.refresh(true);
            let (_, _, total_up, total_down) = filter_network(&networks);

            network_info = NetworkInfo {
                latest_tx: total_up,
                latest_rx: total_down,
                ..network_info  // Counter no longer decremented
            };

            memory_update_count += 1;

            // Save to disk periodically
            if memory_update_count >= network_config.network_interval_number {
                if let Err(e) = rewrite_network_info_file(&mut file, network_info.encode()).await {
                    error!("Failed to write network traffic info file: {e}");
                    continue;
                }
                memory_update_count = 0;
            }

            // Check for monthly reset
            if should_reset_monthly(&network_info.last_reset_date, network_info.config.network_reset_day)
                .unwrap_or(false)
            {
                info!("Monthly reset day reached, clearing network statistics");
                if let Err(e) = rewrite_network_info_file(&mut file, String::new()).await {
                    error!("Failed to clear network traffic info file: {e}");
                }
                break;  // Exit inner loop to reinitialize
            }

            if let Err(e) = tx
                .send((
                    network_info.latest_tx + network_info.source_tx,
                    network_info.latest_rx + network_info.source_rx,
                ))
                .await
            {
                error!("Failed to send traffic data: {e}");
                continue;
            }

            tokio::time::sleep(Duration::from_secs(
                network_info.config.network_interval as u64,
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
