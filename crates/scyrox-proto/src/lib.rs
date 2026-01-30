//! Protobuf definitions for Scyrox daemon IPC.
//!
//! This crate provides the gRPC service definitions and message types
//! for communication between the `scyroxctl` CLI and the `scyroxd` daemon.

/// Generated protobuf types and gRPC service definitions.
pub mod proto {
    tonic::include_proto!("scyrox");
}

// Re-export commonly used types at crate root
pub use proto::scyrox_client::ScyroxClient;
pub use proto::scyrox_server::{Scyrox, ScyroxServer};
pub use proto::*;
