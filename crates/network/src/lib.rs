//! # Phylon Network
//!
//! Remote control WebSocket server and multi-user collaboration session
//! management.
//!
//! Provides a `tokio-tungstenite` WebSocket API for remote simulation control,
//! live observation, and multi-user research sessions.
//!
//! ## Phase 0 scope
//!
//! Placeholder. Implementation: Phase 12.

#![warn(missing_docs)]
#![warn(clippy::all)]

/// Errors from the network subsystem.
#[derive(Debug, thiserror::Error)]
pub enum NetworkError {
    /// The WebSocket server could not bind to the requested address.
    #[error("failed to bind WebSocket server: {message}")]
    BindFailed {
        /// Description of the failure.
        message: String,
    },
}

impl common::PhylonError for NetworkError {}

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::Message;

/// Commands received from RL agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MarlCommand {
    /// Advance the simulation by a fixed number of ticks.
    Step {
        /// Number of simulation ticks to advance.
        ticks: u32,
    },
    /// Request the current state observations.
    GetState,
    /// Apply actions to the simulation actuators.
    SetActions {
        /// Continuous action values.
        actions: Vec<f32>,
    },
    /// Reset the environment to its initial state.
    Reset,
}

/// Responses sent to RL agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MarlResponse {
    /// The current state of the environment.
    State {
        /// The flattened vector of observable values.
        observables: Vec<f32>,
    },
    /// Acknowledgment of a successful operation.
    Ok,
    /// An error occurred during processing.
    Error {
        /// A description of the error.
        message: String,
    },
}

/// A request from the network server to the simulation loop.
pub struct MarlRequest {
    /// The command requested by the RL agent.
    pub command: MarlCommand,
    /// Channel to send the response back to the network server.
    pub reply: tokio::sync::oneshot::Sender<MarlResponse>,
}

/// WebSocket network server for MARL headless control.
pub struct NetworkServer {
    addr: String,
    cmd_tx: tokio::sync::mpsc::Sender<MarlRequest>,
}

impl NetworkServer {
    /// Creates a new network server.
    pub fn new(addr: impl Into<String>, cmd_tx: tokio::sync::mpsc::Sender<MarlRequest>) -> Self {
        Self {
            addr: addr.into(),
            cmd_tx,
        }
    }

    /// Starts the WebSocket server.
    pub async fn start(self) -> Result<(), NetworkError> {
        let listener =
            TcpListener::bind(&self.addr)
                .await
                .map_err(|e| NetworkError::BindFailed {
                    message: e.to_string(),
                })?;

        tracing::info!("Network server listening on ws://{}", self.addr);

        while let Ok((stream, _)) = listener.accept().await {
            let cmd_tx = self.cmd_tx.clone();
            tokio::spawn(async move {
                if let Err(e) = handle_connection(stream, cmd_tx).await {
                    tracing::error!("WebSocket connection error: {}", e);
                }
            });
        }
        Ok(())
    }
}

async fn handle_connection(
    stream: TcpStream,
    cmd_tx: tokio::sync::mpsc::Sender<MarlRequest>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut ws_stream = tokio_tungstenite::accept_async(stream).await?;

    while let Some(msg) = ws_stream.next().await {
        let msg = msg?;
        if msg.is_text() {
            let text = msg.to_text()?;
            let cmd: MarlCommand = match serde_json::from_str(text) {
                Ok(c) => c,
                Err(e) => {
                    let err_resp = MarlResponse::Error {
                        message: e.to_string(),
                    };
                    let resp_str = serde_json::to_string(&err_resp)?;
                    ws_stream.send(Message::Text(resp_str.into())).await?;
                    continue;
                }
            };

            let (res_tx, res_rx) = tokio::sync::oneshot::channel();
            let req = MarlRequest {
                command: cmd,
                reply: res_tx,
            };

            if cmd_tx.send(req).await.is_ok() {
                if let Ok(response) = res_rx.await {
                    let resp_str = serde_json::to_string(&response)?;
                    ws_stream.send(Message::Text(resp_str.into())).await?;
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn network_server_creates() {
        let (tx, _rx) = tokio::sync::mpsc::channel(1);
        let _srv = NetworkServer::new("127.0.0.1:0", tx);
    }
}
