use crate::callbacks::exec::exec_command;
use crate::callbacks::ping::ping_target;
use crate::callbacks::pty::{get_pty_ws_link, handle_pty_session};
use crate::config::UserConfig;
use crate::utils::{ConnectionUrls, connect_ws};
use futures::stream::{SplitSink, SplitStream};
use futures::{SinkExt, StreamExt};
use log::{error, info};
use miniserde::{Deserialize, Serialize, json};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::{Message, Utf8Bytes};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

pub mod exec;
pub mod ping;
pub mod pty;

#[derive(Serialize, Deserialize)]
struct Msg {
    message: String,
}

type Reader = SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>;
type LockedWriter = Arc<Mutex<SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>>;

pub async fn handle_callbacks(
    config: &UserConfig,
    connection_urls: &ConnectionUrls,
    reader: &mut Reader,
    locked_writer: &LockedWriter,
) -> () {
    while let Some(msg) = reader.next().await {
        let Ok(msg) = msg else {
            continue;
        };

        let Ok(utf8) = msg.into_text() else {
            continue;
        };

        info!("Received message from main server: {}", utf8.as_str());

        let json: Msg = if let Ok(value) = json::from_str(utf8.as_str()) {
            value
        } else {
            continue;
        };

        let utf8_cloned = utf8.clone();

        match json.message.as_str() {
            "exec" => {
                if config.terminal {
                    tokio::spawn({
                        let utf8_cloned_for_exec = utf8_cloned.clone();
                        let exec_callback_url = connection_urls.exec_callback.clone();
                        let ignore_unsafe_cert = config.ignore_unsafe_cert;

                        async move {
                            if let Err(e) = exec_command(
                                &utf8_cloned_for_exec,
                                exec_callback_url,
                                &ignore_unsafe_cert,
                            )
                            .await
                            {
                                error!("Exec Error: {e}");
                            }
                        }
                    });
                } else {
                    error!("Terminal feature is disabled");
                }
            }

            "ping" => {
                let locked_write_for_ping = locked_writer.clone();
                tokio::spawn(async move {
                    match ping_target(&utf8_cloned).await {
                        Ok(json_res) => {
                            let mut write = locked_write_for_ping.lock().await;
                            info!("Ping successful: {}", json::to_string(&json_res));
                            if let Err(e) = write
                                .send(Message::Text(Utf8Bytes::from(json::to_string(&json_res))))
                                .await
                            {
                                error!(
                                    "Error occurred while pushing ping result, attempting to reconnect: {e}"
                                );
                            }
                        }
                        Err(err) => {
                            error!("Ping Error: {err}");
                        }
                    }
                });
            }

            "terminal" => {
                if config.terminal {
                    let ws_terminal_url = connection_urls.clone().ws_terminal.clone();
                    let config = config.clone();
                    let utf8_cloned = utf8_cloned.clone();

                    tokio::spawn(async move {
                        let ws_url = match get_pty_ws_link(&utf8_cloned, &ws_terminal_url) {
                            Ok(ws_url) => ws_url,
                            Err(e) => {
                                error!("Failed to get PTY WebSocket URL: {e}");
                                return;
                            }
                        };

                        let ws_stream =
                            match connect_ws(&ws_url, config.tls, config.ignore_unsafe_cert).await {
                                Ok(ws_stream) => ws_stream,
                                Err(e) => {
                                    error!("Failed to connect to PTY WebSocket: {e}");
                                    return;
                                }
                            };

                        if let Err(e) = handle_pty_session(ws_stream, &config.terminal_entry).await {
                            error!("PTY WebSocket handling error: {e}");
                        }
                    });
                } else {
                    error!("Terminal feature is disabled");
                }
            }
            _ => {}
        }
    }
}
