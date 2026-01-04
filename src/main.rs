#![warn(clippy::all, clippy::pedantic)]
#![allow(
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::similar_names,
    clippy::too_many_lines
)]

use crate::callbacks::handle_callbacks;
use crate::command_parser::Args;
use crate::config::{ConfigPath, ConfigReader};
use crate::data_struct::{BasicInfo, RealTimeInfo};
use crate::dry_run::dry_run;
use crate::get_info::network::network_saver::network_saver;
use crate::utils::{build_urls, connect_ws, init_logger};
use futures::stream::SplitSink;
use futures::{SinkExt, StreamExt};
use log::{debug, error, info};
use miniserde::json;
use std::process::exit;
use std::sync::Arc;
use std::time::Duration;
use sysinfo::{CpuRefreshKind, DiskRefreshKind, Disks, MemoryRefreshKind, Networks, RefreshKind};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::time::sleep;
use tokio_tungstenite::tungstenite::{Message, Utf8Bytes};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

mod callbacks;
mod command_parser;
mod config;
mod data_struct;
mod dry_run;
mod get_info;
mod rustls_config;
mod utils;

#[tokio::main]
async fn main() {
    let args = Args::par();

    // Load configuration file
    let config_path = ConfigPath::user_config(args.config.as_deref())
        .unwrap_or_else(|e| {
            eprintln!("Failed to determine config path: {}", e);
            exit(1);
        });

    let config = ConfigReader::load_user_config(&config_path)
        .unwrap_or_else(|e| {
            eprintln!("Error loading configuration: {}", e);
            eprintln!("Please create the configuration file or run 'kagent.sh config' to configure.");
            exit(1);
        });

    // Initialize logger with config
    init_logger(&config.log_level);

    // Set global DURATION variable
    unsafe {
        crate::get_info::network::DURATION = config.realtime_info_interval as f64;
    }

    dry_run().await;

    if args.dry_run {
        exit(0);
    }

    debug!("Configuration loaded from: {}", config_path.display());
    debug!("HTTP Server: {}", config.http_server);
    debug!("Token: [REDACTED]");
    debug!("TLS: {}", config.tls);
    debug!("Network Statistics: {}", !config.disable_network_statistics);

    let connection_urls = build_urls(
        &config.http_server,
        config.ws_server.as_ref(),
        &config.token,
    )
    .unwrap_or_else(|e| {
        error!("Failed to parse server address: {e}");
        exit(1);
    });

    for line in connection_urls.to_string().lines() {
        debug!("{line}");
    }

    #[cfg(target_os = "windows")]
    {
        if !config.disable_toast_notify {
            use win_toast_notify::{Action, ActivationType, WinToastNotify};
            WinToastNotify::new()
                .set_title("Komari-monitor-rs Is Running!")
                .set_messages(vec![
                    "Komari-monitor-rs is an application used to monitor your system, granting it near-complete access to your computer. If you did not actively install this program, please check your system immediately. If you have intentionally used this software on your system, please ignore this message or set `disable_toast_notify=true` in the configuration file."
                ])
                .set_actions(vec![
                    Action {
                        activation_type: ActivationType::Protocol,
                        action_content: "komari-monitor".to_string(),
                        arguments: "https://github.com/komari-monitor".to_string(),
                        image_url: None
                    },
                    Action {
                        activation_type: ActivationType::Protocol,
                        action_content: "komari-monitor-rs".to_string(),
                        arguments: "https://github.com/NEOherozzz/komari-monitor-rs".to_string(),
                        image_url: None
                    },
                ])
                .show()
                .expect("Failed to show toast notification");
        }
    }

    let (network_saver_tx, mut network_saver_rx): (Sender<(u64, u64)>, Receiver<(u64, u64)>) =
        tokio::sync::mpsc::channel(15);

    if !config.disable_network_statistics {
        let config_clone = config.clone();
        let config_path_clone = config_path.clone();
        let _listener = tokio::spawn(async move {
            network_saver(network_saver_tx, &config_clone, &config_path_clone).await;
        });
    } else {
        info!(
            "Network statistics feature disabled. This will fallback to statistics only showing network interface traffic since the current startup"
        );
    }

    loop {
        let Ok(ws_stream) = connect_ws(
            &connection_urls.ws_real_time,
            config.tls,
            config.ignore_unsafe_cert,
        )
        .await
        else {
            error!("Failed to connect to WebSocket server, retrying in 5 seconds");
            sleep(Duration::from_secs(5)).await;
            continue;
        };

        let (write, mut read) = ws_stream.split();

        let locked_write: Arc<
            Mutex<SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>,
        > = Arc::new(Mutex::new(write));

        // Handle callbacks
        {
            let config_cloned = config.clone();
            let connection_urls_cloned = connection_urls.clone();
            let locked_write_cloned = locked_write.clone();
            let _listener = tokio::spawn(async move {
                handle_callbacks(
                    &config_cloned,
                    &connection_urls_cloned,
                    &mut read,
                    &locked_write_cloned,
                )
                .await;
            });
        }

        let mut sysinfo_sys = sysinfo::System::new();
        let mut networks = Networks::new_with_refreshed_list();
        let mut disks = Disks::new();
        sysinfo_sys.refresh_cpu_list(
            CpuRefreshKind::nothing()
                .without_cpu_usage()
                .without_frequency(),
        );
        sysinfo_sys.refresh_memory_specifics(MemoryRefreshKind::everything());

        let basic_info = BasicInfo::build(&sysinfo_sys, config.fake, &config.ip_provider).await;

        basic_info.push(connection_urls.basic_info.clone(), config.ignore_unsafe_cert);

        loop {
            let start_time = tokio::time::Instant::now();
            sysinfo_sys.refresh_specifics(
                RefreshKind::nothing()
                    .with_cpu(CpuRefreshKind::everything().without_frequency())
                    .with_memory(MemoryRefreshKind::everything()),
            );
            networks.refresh(true);
            disks.refresh_specifics(true, DiskRefreshKind::nothing().with_storage());
            let real_time = RealTimeInfo::build(
                &sysinfo_sys,
                &networks,
                if config.disable_network_statistics {
                    None
                } else {
                    Some(&mut network_saver_rx)
                },
                &disks,
                config.fake,
            );

            let json = json::to_string(&real_time);
            {
                let mut write = locked_write.lock().await;
                if let Err(e) = write.send(Message::Text(Utf8Bytes::from(json))).await {
                    error!(
                        "Error occurred while pushing RealTime Info, attempting to reconnect: {e}"
                    );
                    break;
                }
            }
            let end_time = start_time.elapsed();

            sleep(Duration::from_millis({
                let end = u64::try_from(end_time.as_millis()).unwrap_or(0);
                config.realtime_info_interval.saturating_sub(end)
            }))
            .await;
        }
    }
}
