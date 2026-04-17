//! `sidex-dap` — Debug Adapter Protocol client for `SideX`.
//!
//! Implements the [DAP specification](https://microsoft.github.io/debug-adapter-protocol/)
//! with full protocol types, Content-Length framed transport, a high-level
//! debug client, session state tracking, adapter registry, and launch
//! configuration parsing.

pub mod adapter;
pub mod client;
pub mod launch_config;
pub mod protocol;
pub mod session;
pub mod transport;

pub use adapter::{DebugAdapterDescriptor, DebugAdapterRegistry};
pub use client::DebugClient;
pub use launch_config::{
    builtin_templates, parse_launch_json, AttachConfig, CompoundLaunchConfig, ConsoleType,
    LaunchConfig, LaunchConfigTemplate,
};
pub use protocol::*;
pub use session::{BreakpointPersistence, DebugSession, SessionState};
pub use transport::DapTransport;
