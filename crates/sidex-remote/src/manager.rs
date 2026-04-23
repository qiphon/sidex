//! Remote connection manager.
//!
//! [`RemoteManager`] keeps track of all active remote connections regardless
//! of backend and provides a uniform `ConnectionId`-based API for the rest
//! of `SideX` to interact with them.

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use crate::codespaces::CodespacesTransport;
use crate::container::{parse_devcontainer, ContainerTransport};
use crate::ssh::{SshAuth, SshTransport};
use crate::transport::RemoteTransport;
use crate::wsl::WslTransport;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Opaque handle to an active remote connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConnectionId(pub u64);

/// Which backend a connection uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionKind {
    Ssh,
    Wsl,
    Container,
    Codespace,
    Tunnel,
}

/// Public metadata for a live connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    pub id: ConnectionId,
    pub kind: ConnectionKind,
    pub label: String,
    /// Seconds since the connection was established.
    pub connected_secs: u64,
}

// ---------------------------------------------------------------------------
// Internal wrapper
// ---------------------------------------------------------------------------

struct Entry {
    transport: Box<dyn RemoteTransport>,
    kind: ConnectionKind,
    label: String,
    connected_at: Instant,
}

// ---------------------------------------------------------------------------
// Manager
// ---------------------------------------------------------------------------

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// Manages the lifecycle of all remote connections.
pub struct RemoteManager {
    connections: HashMap<ConnectionId, Entry>,
}

impl Default for RemoteManager {
    fn default() -> Self {
        Self::new()
    }
}

impl RemoteManager {
    /// Create an empty manager with no active connections.
    pub fn new() -> Self {
        Self {
            connections: HashMap::new(),
        }
    }

    fn insert(
        &mut self,
        transport: Box<dyn RemoteTransport>,
        kind: ConnectionKind,
        label: String,
    ) -> ConnectionId {
        let id = ConnectionId(NEXT_ID.fetch_add(1, Ordering::Relaxed));
        self.connections.insert(
            id,
            Entry {
                transport,
                kind,
                label,
                connected_at: Instant::now(),
            },
        );
        id
    }

    // -- SSH ----------------------------------------------------------------

    /// Open an SSH connection.
    pub async fn connect_ssh(
        &mut self,
        host: &str,
        port: u16,
        auth: SshAuth,
    ) -> Result<ConnectionId> {
        let transport = SshTransport::connect(host, port, auth).await?;
        let label = format!("{host}:{port}");
        Ok(self.insert(Box::new(transport), ConnectionKind::Ssh, label))
    }

    /// Open an SSH connection as a specific user with optional keepalive.
    pub async fn connect_ssh_as(
        &mut self,
        user: &str,
        host: &str,
        port: u16,
        auth: SshAuth,
        keepalive_secs: Option<u64>,
    ) -> Result<ConnectionId> {
        let transport = SshTransport::connect_as(user, host, port, auth, keepalive_secs).await?;
        let label = format!("{user}@{host}:{port}");
        Ok(self.insert(Box::new(transport), ConnectionKind::Ssh, label))
    }

    // -- WSL ----------------------------------------------------------------

    /// Connect to a WSL distribution.
    pub async fn connect_wsl(&mut self, distro: &str) -> Result<ConnectionId> {
        let transport = WslTransport::connect(distro).await?;
        let label = format!("WSL: {distro}");
        Ok(self.insert(Box::new(transport), ConnectionKind::Wsl, label))
    }

    // -- Container ----------------------------------------------------------

    /// Start and connect to a dev container from a `devcontainer.json` path.
    pub async fn connect_container(&mut self, config_path: &Path) -> Result<ConnectionId> {
        let config = parse_devcontainer(config_path)?;
        let workspace = config_path
            .parent()
            .and_then(Path::parent)
            .unwrap_or(config_path);
        let transport = ContainerTransport::start(&config, workspace).await?;
        let label = format!("Container: {}", config_path.display());
        Ok(self.insert(Box::new(transport), ConnectionKind::Container, label))
    }

    // -- Codespace ----------------------------------------------------------

    /// Connect to a running GitHub Codespace.
    pub async fn connect_codespace(&mut self, name: &str, token: &str) -> Result<ConnectionId> {
        let transport = CodespacesTransport::connect(name, token).await?;
        let label = format!("Codespace: {name}");
        Ok(self.insert(Box::new(transport), ConnectionKind::Codespace, label))
    }

    // -- Tunnel -------------------------------------------------------------

    /// Connect via a remote tunnel (placeholder — requires a `TunnelClient`
    /// to be wired into `RemoteTransport`).
    #[allow(clippy::unused_async)]
    pub async fn connect_tunnel(&mut self, _tunnel_id: &str) -> Result<ConnectionId> {
        bail!("tunnel transport not yet wired into RemoteTransport")
    }

    // -- General ------------------------------------------------------------

    /// Disconnect and remove a connection.
    pub async fn disconnect(&mut self, id: ConnectionId) -> Result<()> {
        let entry = self
            .connections
            .remove(&id)
            .ok_or_else(|| anyhow::anyhow!("no connection with id {}", id.0))?;
        entry.transport.disconnect().await?;
        Ok(())
    }

    /// Retrieve a reference to the transport behind a connection.
    pub fn get(&self, id: ConnectionId) -> Option<&dyn RemoteTransport> {
        self.connections.get(&id).map(|e| &*e.transport)
    }

    /// List all active connections.
    pub fn active_connections(&self) -> Vec<ConnectionInfo> {
        self.connections
            .iter()
            .map(|(&id, entry)| ConnectionInfo {
                id,
                kind: entry.kind,
                label: entry.label.clone(),
                connected_secs: entry.connected_at.elapsed().as_secs(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manager_starts_empty() {
        let mgr = RemoteManager::new();
        assert!(mgr.active_connections().is_empty());
    }

    #[test]
    fn connection_id_equality() {
        let a = ConnectionId(1);
        let b = ConnectionId(1);
        let c = ConnectionId(2);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn get_missing_returns_none() {
        let mgr = RemoteManager::new();
        assert!(mgr.get(ConnectionId(999)).is_none());
    }
}
