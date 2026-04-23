//! Remote development for `SideX` — SSH, WSL, Dev Containers, Codespaces, Tunnels.
//!
//! This crate provides a unified [`transport::RemoteTransport`] trait that
//! every backend implements.  The [`manager::RemoteManager`] keeps track of
//! active connections and the [`server::SideXServer`] runs on the remote
//! machine to service JSON-RPC requests.

pub mod codespaces;
pub mod container;
pub mod manager;
pub mod port_forwarding;
pub mod server;
pub mod ssh;
pub mod transport;
pub mod tunnel;
pub mod wsl;

pub use manager::{ConnectionId, ConnectionInfo, ConnectionKind, RemoteManager};
pub use port_forwarding::{ForwardedPort, PortForwardingService, PortPrivacy, PortProtocol};
pub use transport::{DirEntry, ExecOutput, FileStat, RemotePty, RemoteTransport};
