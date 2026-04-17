//! Automatic port forwarding service.
//!
//! Detects when a remote process starts listening on a port, notifies the
//! user, and optionally forwards it to localhost.  Supports both auto-detected
//! and manually configured forwards.

use std::collections::HashSet;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::transport::{ExecOutput, RemoteTransport};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PortProtocol {
    Http,
    Https,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PortPrivacy {
    Private,
    Public,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForwardedPort {
    pub local_port: u16,
    pub remote_port: u16,
    pub process_name: Option<String>,
    pub label: Option<String>,
    pub protocol: PortProtocol,
    pub privacy: PortPrivacy,
    pub is_auto_detected: bool,
    pub is_active: bool,
}

/// Notification emitted when auto-detection discovers a new listening port.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortDetectedEvent {
    pub remote_port: u16,
    pub process_name: Option<String>,
    pub suggested_label: Option<String>,
}

/// Manages port forwards for a single remote connection.
pub struct PortForwardingService {
    pub forwards: Vec<ForwardedPort>,
    pub auto_detect: bool,
    known_ports: HashSet<u16>,
}

impl Default for PortForwardingService {
    fn default() -> Self {
        Self::new()
    }
}

impl PortForwardingService {
    pub fn new() -> Self {
        Self {
            forwards: Vec::new(),
            auto_detect: true,
            known_ports: HashSet::new(),
        }
    }

    /// Detect listening ports on the remote machine by parsing `ss` output.
    pub async fn detect_listening_ports(
        &self,
        transport: &dyn RemoteTransport,
    ) -> Result<Vec<u16>> {
        let out: ExecOutput = transport
            .exec("ss -tlnp 2>/dev/null || netstat -tlnp 2>/dev/null || echo ''")
            .await?;

        let mut ports = Vec::new();
        for line in out.stdout.lines().skip(1) {
            for token in line.split_whitespace() {
                if let Some(port_str) = token.rsplit(':').next() {
                    if let Ok(port) = port_str.parse::<u16>() {
                        if port > 0 && !ports.contains(&port) {
                            ports.push(port);
                        }
                    }
                }
            }
        }
        Ok(ports)
    }

    /// Scan for newly opened ports and return events for any that are new.
    pub async fn poll_new_ports(
        &mut self,
        transport: &dyn RemoteTransport,
    ) -> Result<Vec<PortDetectedEvent>> {
        let current = self.detect_listening_ports(transport).await?;
        let mut events = Vec::new();

        for &port in &current {
            if self.known_ports.insert(port) {
                let label = guess_label(port);
                events.push(PortDetectedEvent {
                    remote_port: port,
                    process_name: None,
                    suggested_label: label,
                });
            }
        }

        self.known_ports.retain(|p| current.contains(p));
        Ok(events)
    }

    /// Manually add a port forward.
    pub fn forward_port(
        &mut self,
        local_port: u16,
        remote_port: u16,
    ) -> &ForwardedPort {
        let idx = self.forwards.iter().position(|f| f.remote_port == remote_port);
        if let Some(i) = idx {
            self.forwards[i].local_port = local_port;
            self.forwards[i].is_active = true;
            return &self.forwards[i];
        }
        self.forwards.push(ForwardedPort {
            local_port,
            remote_port,
            process_name: None,
            label: guess_label(remote_port),
            protocol: guess_protocol(remote_port),
            privacy: PortPrivacy::Private,
            is_auto_detected: false,
            is_active: true,
        });
        self.forwards.last().unwrap()
    }

    /// Record an auto-detected port forward.
    pub fn record_auto_forward(&mut self, port: u16) -> &ForwardedPort {
        let idx = self.forwards.iter().position(|f| f.remote_port == port);
        if let Some(i) = idx {
            self.forwards[i].is_active = true;
            self.forwards[i].is_auto_detected = true;
            return &self.forwards[i];
        }
        self.forwards.push(ForwardedPort {
            local_port: port,
            remote_port: port,
            process_name: None,
            label: guess_label(port),
            protocol: guess_protocol(port),
            privacy: PortPrivacy::Private,
            is_auto_detected: true,
            is_active: true,
        });
        self.forwards.last().unwrap()
    }

    /// Remove / deactivate a specific port forward.
    pub fn remove_forward(&mut self, remote_port: u16) -> bool {
        if let Some(fwd) = self.forwards.iter_mut().find(|f| f.remote_port == remote_port) {
            fwd.is_active = false;
            true
        } else {
            false
        }
    }

    /// Get an active forward by remote port.
    pub fn get(&self, remote_port: u16) -> Option<&ForwardedPort> {
        self.forwards
            .iter()
            .find(|f| f.remote_port == remote_port && f.is_active)
    }

    /// Generate the URL to open in a browser for a given port.
    pub fn browser_url(&self, port: u16) -> Result<String> {
        let fwd = self
            .get(port)
            .ok_or_else(|| anyhow::anyhow!("no active forward for port {port}"))?;
        let scheme = match fwd.protocol {
            PortProtocol::Https => "https",
            _ => "http",
        };
        Ok(format!("{scheme}://localhost:{}", fwd.local_port))
    }

    /// List all currently active forwards.
    pub fn active_forwards(&self) -> Vec<&ForwardedPort> {
        self.forwards.iter().filter(|f| f.is_active).collect()
    }
}

// ---------------------------------------------------------------------------
// Heuristics
// ---------------------------------------------------------------------------

fn guess_protocol(port: u16) -> PortProtocol {
    match port {
        443 | 8443 => PortProtocol::Https,
        80 | 3000 | 4200 | 5000 | 5173 | 5174 | 8000 | 8080 | 8888 | 9000 => PortProtocol::Http,
        _ => PortProtocol::Custom,
    }
}

fn guess_label(port: u16) -> Option<String> {
    let label = match port {
        80 => "HTTP",
        443 => "HTTPS",
        3000 => "Dev Server (3000)",
        4200 => "Angular (4200)",
        5000 => "Flask / API (5000)",
        5173 | 5174 => "Vite (5173)",
        8000 => "Django (8000)",
        8080 => "HTTP Proxy (8080)",
        8888 => "Jupyter (8888)",
        9000 => "PHP / Debug (9000)",
        _ => return None,
    };
    Some(label.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_and_retrieve() {
        let mut svc = PortForwardingService::new();
        svc.forward_port(3001, 3000);
        assert!(svc.get(3000).is_some());
        assert_eq!(svc.get(3000).unwrap().local_port, 3001);
    }

    #[test]
    fn remove_forward() {
        let mut svc = PortForwardingService::new();
        svc.forward_port(8080, 8080);
        assert!(svc.remove_forward(8080));
        assert!(svc.get(8080).is_none());
    }

    #[test]
    fn auto_forward_recording() {
        let mut svc = PortForwardingService::new();
        svc.record_auto_forward(3000);
        let fwd = svc.get(3000).unwrap();
        assert!(fwd.is_auto_detected);
        assert_eq!(fwd.label.as_deref(), Some("Dev Server (3000)"));
    }

    #[test]
    fn browser_url_generation() {
        let mut svc = PortForwardingService::new();
        svc.forward_port(3000, 3000);
        assert_eq!(svc.browser_url(3000).unwrap(), "http://localhost:3000");
    }

    #[test]
    fn guess_protocol_and_label() {
        assert_eq!(guess_protocol(443), PortProtocol::Https);
        assert_eq!(guess_protocol(3000), PortProtocol::Http);
        assert_eq!(guess_protocol(12345), PortProtocol::Custom);
        assert!(guess_label(3000).is_some());
        assert!(guess_label(12345).is_none());
    }
}
