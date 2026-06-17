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

/// Placeholder for the network server.
///
/// TODO(phase-12): Implement tokio-tungstenite WebSocket API for remote
/// control and collaboration.
pub struct NetworkServer;

impl NetworkServer {
    /// Creates a placeholder network server.
    pub fn placeholder() -> Self {
        Self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn network_server_placeholder_creates() {
        let _srv = NetworkServer::placeholder();
    }
}
