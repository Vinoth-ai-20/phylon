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

/// # Multi-Agent Reinforcement Learning Command
///
/// ## 1. What Happens
/// The `MarlCommand` enum defines the typed RPC payload sent from an external Machine Learning
/// trainer (like a Python PPO script) over the WebSocket to the Phylon engine.
///
/// ## 2. Why It Happens
/// In headless ML training, the engine acts as an OpenAI Gym Environment. It cannot run continuously;
/// it must wait for the Python script to run its backward pass, update gradients, and compute the
/// next deterministic action vector before advancing physics. This enum forces synchronous lock-stepping.
///
/// ## 3. How It Happens
/// The payload is serialized as JSON string frames over TCP:
/// 1. `Step { ticks }` unpauses the engine loop for $N$ iterations.
/// 2. `SetActions` injects the network output tensor $A_t$ into the organisms.
/// 3. `GetState` requests the latest $O_t$ observation tensor.
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

/// # Multi-Agent Reinforcement Learning Response
///
/// ## 1. What Happens
/// The `MarlResponse` enum defines the typed RPC payload sent from the Phylon engine back to
/// the external Machine Learning trainer in response to a `MarlCommand`.
///
/// ## 2. Why It Happens
/// When the trainer requests `GetState` after executing a `Step`, it expects the simulation to
/// have resolved physics, evaluated collisions, and computed the new sensor values. This response
/// delivers that data.
///
/// ## 3. How It Happens
/// The `State` variant serializes the continuous $\mathbb{R}^N$ `ObservationVector` from the
/// `learning` crate back into JSON format and pushes it down the WebSocket stream.
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

/// # Async WebSocket Network Server
///
/// ## 1. What Happens
/// The `NetworkServer` manages a headless TCP WebSocket listener using `tokio` and `tungstenite`.
/// It acts as the networking bridge between the asynchronous IO runtime and the synchronous Bevy ECS.
///
/// ## 2. Why It Happens
/// Phylon's core simulation (`crates/app`) is a blocking `winit` event loop or a tight `while` loop
/// (headless). Networking must not block the physics integration. Therefore, we spawn a separate
/// `tokio` runtime that handles TCP handshakes and JSON parsing, passing strongly-typed
/// `MarlRequest` structs to the ECS via a lock-free MPSC channel.
///
/// ## 3. How It Happens
/// The server listens on `addr`. When a connection is accepted, a green thread (`tokio::spawn`)
/// parses incoming `Message::Text` frames into `MarlCommand`s, constructs a one-shot reply channel,
/// and sends the job to `cmd_tx`. The ECS dequeues the job during its pre-tick phase.
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
