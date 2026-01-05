use crate::config::{ConfigPath, ConfigReader, RuntimeData, TrafficMode, UserConfig};
use crate::get_info::network::filter_network;
use log::{error, info, warn};
use std::fs;
use std::io::SeekFrom;
use std::path::PathBuf;
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

/// Calculate last_reset_month for initial setup
/// If current day >= reset_day, the reset day has passed this month, so last_reset_month = current_month
/// If current day < reset_day, the reset day hasn't arrived this month, so last_reset_month = previous_month
fn calculate_initial_last_reset_month(reset_day: u8) -> u8 {
    let current_month = get_current_month();
    let current_day = get_current_day();
    let effective_reset_day = get_effective_reset_day(reset_day);

    if current_day >= effective_reset_day {
        // Reset day has passed this month
        current_month
    } else {
        // Reset day hasn't arrived this month, set to previous month
        if current_month == 1 {
            12
        } else {
            current_month - 1
        }
    }
}

/// Check if traffic should be reset based on current date and reset configuration
fn should_reset_traffic(last_reset_month: u8, reset_day: u8) -> bool {
    let current_month = get_current_month();
    let current_day = get_current_day();
    let effective_reset_day = get_effective_reset_day(reset_day);

    if current_month == last_reset_month {
        return false; // Same month, no reset
    }

    // Calculate month difference
    let months_diff = if current_month > last_reset_month {
        current_month - last_reset_month
    } else {
        12 - last_reset_month + current_month // Cross year
    };

    // Reset if:
    // 1. More than 1 month has passed (handles long downtime)
    // 2. Exactly 1 month has passed and current day >= reset_day
    months_diff > 1 || current_day >= effective_reset_day
}

async fn get_or_init_runtime_data(
    runtime_data_path: &PathBuf,
    reset_day: u8,
) -> Result<(File, RuntimeData), String> {
    let mut file = match OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(runtime_data_path)
        .await
    {
        Ok(file) => file,
        Err(e) => {
            return Err(format!("Failed to open runtime data file: {e}"));
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
        .map_err(|e| format!("Failed to read runtime data file: {e}"))?;

    let raw_runtime_data = if raw_data.is_empty() {
        let runtime_data = RuntimeData {
            boot_id: new_boot_id.clone(),
            boot_source_tx: 0,
            boot_source_rx: 0,
            current_boot_tx: 0,
            current_boot_rx: 0,
            accumulated_tx: 0,
            accumulated_rx: 0,
            last_reset_month: calculate_initial_last_reset_month(reset_day),
        };
        rewrite_runtime_data_file(&mut file, &runtime_data)
            .await
            .map_err(|e| format!("Failed to write runtime data file: {e}"))?;
        info!("Runtime data file is empty, created new file");
        runtime_data
    } else {
        let raw_runtime_data = RuntimeData::decode(&raw_data);

        if let Err(e) = &raw_runtime_data {
            warn!(
                "Failed to parse runtime data file: {}. Will recreate the file in 3 seconds.",
                e
            );
            tokio::time::sleep(Duration::from_secs(3)).await;

            drop(file);
            if let Err(e) = tokio::fs::remove_file(runtime_data_path).await {
                return Err(format!(
                    "Failed to remove corrupted runtime data file: {e}"
                ));
            }

            file = reopen_runtime_data_file(runtime_data_path).await?;

            let runtime_data = RuntimeData {
                boot_id: new_boot_id.clone(),
                boot_source_tx: 0,
                boot_source_rx: 0,
                current_boot_tx: 0,
                current_boot_rx: 0,
                accumulated_tx: 0,
                accumulated_rx: 0,
                last_reset_month: calculate_initial_last_reset_month(reset_day),
            };
            rewrite_runtime_data_file(&mut file, &runtime_data)
                .await
                .map_err(|e| format!("Failed to write runtime data file: {e}"))?;
            info!("Recreated runtime data file");
            runtime_data
        } else {
            raw_runtime_data?
        }
    };

    // Handle system reboot: merge current boot traffic into accumulated
    let new_runtime_data = if cfg!(target_os = "linux") && !new_boot_id.is_empty() {
        if raw_runtime_data.boot_id != new_boot_id {
            info!("System reboot detected, merging traffic data");
            let runtime_data = RuntimeData {
                boot_id: new_boot_id,
                boot_source_tx: 0,  // New boot starts from 0
                boot_source_rx: 0,
                current_boot_tx: 0,  // Clear current boot
                current_boot_rx: 0,
                // Merge last boot traffic into accumulated
                accumulated_tx: raw_runtime_data.accumulated_tx + raw_runtime_data.current_boot_tx,
                accumulated_rx: raw_runtime_data.accumulated_rx + raw_runtime_data.current_boot_rx,
                last_reset_month: raw_runtime_data.last_reset_month,
            };
            rewrite_runtime_data_file(&mut file, &runtime_data)
                .await
                .map_err(|e| format!("Failed to write runtime data file: {e}"))?;
            runtime_data
        } else {
            raw_runtime_data
        }
    } else if cfg!(target_os = "windows") {
        // Windows: always merge on startup as we can't reliably detect reboots
        let runtime_data = RuntimeData {
            boot_id: new_boot_id,
            boot_source_tx: 0,  // Reset to 0 on each program start
            boot_source_rx: 0,
            current_boot_tx: 0,  // Clear current boot
            current_boot_rx: 0,
            // Merge last boot traffic into accumulated
            accumulated_tx: raw_runtime_data.accumulated_tx + raw_runtime_data.current_boot_tx,
            accumulated_rx: raw_runtime_data.accumulated_rx + raw_runtime_data.current_boot_rx,
            last_reset_month: raw_runtime_data.last_reset_month,
        };
        rewrite_runtime_data_file(&mut file, &runtime_data)
            .await
            .map_err(|e| format!("Failed to write runtime data file: {e}"))?;
        runtime_data
    } else {
        raw_runtime_data
    };

    Ok((file, new_runtime_data))
}

pub async fn network_saver(
    tx: tokio::sync::mpsc::Sender<(u64, u64)>,
    config: &UserConfig,
    config_path: &PathBuf,
) {
    if config.disable_network_statistics {
        return;
    }

    // Get runtime data file path
    let runtime_data_path = match ConfigPath::runtime_data() {
        Ok(path) => path,
        Err(e) => {
            error!("Failed to determine runtime data path: {}", e);
            warn!("Network statistics disabled due to path error");
            return;
        }
    };

    let (mut file, mut runtime_data) = match get_or_init_runtime_data(&runtime_data_path, config.reset_day).await
    {
        Ok(n) => n,
        Err(e) => {
            warn!("An error occurred while getting or initing runtime data: {e}.");
            warn!(
                "This will fallback to statistics only showing network interface traffic since the current startup, equivalent to `disable_network_statistics=true`."
            );
            return;
        }
    };

    let mut networks = Networks::new_with_refreshed_list();
    let mut save_counter = 0u32; // Counter for periodic disk writes

    // Keep a local copy of config for hot-reload
    let mut current_config = config.clone();

    loop {
        // Hot-reload configuration from file
        match ConfigReader::load_user_config(config_path) {
            Ok(new_config) => {
                // Check if network-related settings have changed
                let config_changed =
                    current_config.reset_day != new_config.reset_day ||
                    current_config.calibration_tx != new_config.calibration_tx ||
                    current_config.calibration_rx != new_config.calibration_rx ||
                    current_config.network_interval != new_config.network_interval ||
                    current_config.traffic_mode != new_config.traffic_mode;

                if config_changed {
                    info!("Configuration file changes detected, reloading network settings");

                    if current_config.reset_day != new_config.reset_day {
                        info!("  reset_day: {} -> {}", current_config.reset_day, new_config.reset_day);
                    }
                    if current_config.calibration_tx != new_config.calibration_tx {
                        info!("  calibration_tx: {} -> {}", current_config.calibration_tx, new_config.calibration_tx);
                    }
                    if current_config.calibration_rx != new_config.calibration_rx {
                        info!("  calibration_rx: {} -> {}", current_config.calibration_rx, new_config.calibration_rx);
                    }
                    if current_config.network_interval != new_config.network_interval {
                        info!("  network_interval: {}s -> {}s", current_config.network_interval, new_config.network_interval);
                    }
                    if current_config.traffic_mode != new_config.traffic_mode {
                        info!("  traffic_mode: {:?} -> {:?}", current_config.traffic_mode, new_config.traffic_mode);
                    }

                    current_config = new_config;
                }
            }
            Err(e) => {
                warn!("Failed to reload configuration file: {}, using cached config", e);
            }
        }

        networks.refresh(true);
        let (_, _, total_up, total_down) = filter_network(&networks);

        // Check if we need to reset traffic based on monthly schedule
        if should_reset_traffic(runtime_data.last_reset_month, current_config.reset_day) {
            let current_month = get_current_month();
            let effective_reset_day = get_effective_reset_day(current_config.reset_day);
            info!(
                "Monthly traffic reset triggered (configured day: {}, effective day: {}, current month: {})",
                current_config.reset_day, effective_reset_day, current_month
            );

            runtime_data = RuntimeData {
                boot_id: runtime_data.boot_id.clone(),
                boot_source_tx: total_up,  // Set current system traffic as new baseline
                boot_source_rx: total_down,
                current_boot_tx: 0,        // Clear current boot (new period starts)
                current_boot_rx: 0,
                accumulated_tx: 0,         // Reset accumulated to 0
                accumulated_rx: 0,
                last_reset_month: current_month,
            };

            // Immediately save the reset state
            if let Err(e) = rewrite_runtime_data_file(&mut file, &runtime_data).await {
                error!("Failed to write runtime data file after reset: {e}");
            } else {
                info!("Traffic statistics reset completed");
            }

            // Clear calibration values in config file
            if current_config.calibration_tx != 0 || current_config.calibration_rx != 0 {
                info!("Clearing calibration values in config file");

                // Load current config
                match ConfigReader::load_user_config(config_path) {
                    Ok(mut user_config) => {
                        // Clear calibration values
                        user_config.calibration_tx = 0;
                        user_config.calibration_rx = 0;

                        // Save back to config file
                        match ConfigReader::save_user_config(config_path, &user_config) {
                            Ok(_) => {
                                info!("Calibration values cleared in config file");
                                // Update local config immediately
                                current_config.calibration_tx = 0;
                                current_config.calibration_rx = 0;
                            }
                            Err(e) => {
                                error!("Failed to update config file: {}", e);
                                warn!("Please manually set calibration_tx=0 and calibration_rx=0 in {}",
                                      config_path.display());
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to load config file for calibration reset: {}", e);
                        warn!("Please manually set calibration_tx=0 and calibration_rx=0 in {}",
                              config_path.display());
                    }
                }
            }
        }

        save_counter += 1;

        // Periodically save to disk (every 10 intervals by default)
        if save_counter >= 10 {
            // Calculate current boot traffic cumulative increment
            let current_boot_tx = total_up.saturating_sub(runtime_data.boot_source_tx);
            let current_boot_rx = total_down.saturating_sub(runtime_data.boot_source_rx);

            // Save current boot cumulative increment
            runtime_data.current_boot_tx = current_boot_tx;
            runtime_data.current_boot_rx = current_boot_rx;

            // boot_source remains unchanged (stays constant during the entire boot period)
            // accumulated remains unchanged (only modified on reboot or monthly reset)

            if let Err(e) = rewrite_runtime_data_file(&mut file, &runtime_data).await {
                error!("Failed to write runtime data file: {e}");
            }
            save_counter = 0;
        }

        // Calculate current boot traffic for display
        let current_boot_tx = total_up.saturating_sub(runtime_data.boot_source_tx);
        let current_boot_rx = total_down.saturating_sub(runtime_data.boot_source_rx);

        // Calculate total traffic including calibration values
        let base_tx = runtime_data.accumulated_tx + current_boot_tx + current_config.calibration_tx;
        let base_rx = runtime_data.accumulated_rx + current_boot_rx + current_config.calibration_rx;

        // Apply traffic mode to determine what to send to the main loop
        let (total_tx, total_rx) = match current_config.traffic_mode {
            TrafficMode::Both => (base_tx, base_rx),      // Send both TX and RX
            TrafficMode::TxOnly => (base_tx, 0),          // Only TX counts, set RX to 0
            TrafficMode::RxOnly => (0, base_rx),          // Only RX counts, set TX to 0
        };

        if let Err(e) = tx.send((total_tx, total_rx)).await {
            error!("Failed to send traffic data: {e}");
        }

        tokio::time::sleep(Duration::from_secs(
            current_config.network_interval as u64,
        ))
        .await;

        // Configuration hot-reload is enabled
        // Changes to network settings will be automatically applied in the next iteration
    }
}

async fn rewrite_runtime_data_file(
    file: &mut File,
    runtime_data: &RuntimeData,
) -> Result<(), Box<dyn std::error::Error>> {
    let encoded = runtime_data.encode();
    file.set_len(0).await?;
    file.seek(SeekFrom::Start(0)).await?;
    file.write_all(encoded.as_bytes()).await?;
    Ok(())
}

async fn reopen_runtime_data_file(path: &PathBuf) -> Result<File, String> {
    match OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .await
    {
        Ok(file) => Ok(file),
        Err(e) => Err(format!("Failed to reopen runtime data file: {e}")),
    }
}
