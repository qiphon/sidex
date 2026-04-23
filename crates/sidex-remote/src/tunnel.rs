//! Remote Tunnel transport backend.
//!
//! A `TunnelServer` runs on the remote machine and connects outbound to a
//! relay.  A `TunnelClient` connects to the same relay and the two ends are
//! paired.  Communication happens via JSON-RPC messages over a TLS WebSocket.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::tungstenite::Message;

// ---------------------------------------------------------------------------
// JSON-RPC envelope
// ---------------------------------------------------------------------------

/// Minimal JSON-RPC 2.0 request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcRequest {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

/// Minimal JSON-RPC 2.0 response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcResponse {
    pub jsonrpc: String,
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

/// JSON-RPC error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
}

impl RpcRequest {
    /// Create a new JSON-RPC request.
    pub fn new(id: u64, method: impl Into<String>, params: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            method: method.into(),
            params,
        }
    }
}

impl RpcResponse {
    /// Success response.
    pub fn ok(id: u64, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Error response.
    pub fn err(id: u64, code: i64, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(RpcError {
                code,
                message: message.into(),
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// Tunnel server (runs on the remote machine)
// ---------------------------------------------------------------------------

/// Server side of a remote tunnel.
///
/// Connects outbound to a relay URL and registers itself with the given
/// `auth_token`.  The relay pairs this connection with a [`TunnelClient`].
pub struct TunnelServer {
    write_tx: mpsc::Sender<Message>,
    read_rx: Arc<Mutex<mpsc::Receiver<Message>>>,
    _task: tokio::task::JoinHandle<()>,
}

impl TunnelServer {
    /// Start the tunnel server, connecting to the relay.
    pub async fn start(relay_url: &str, auth_token: &str) -> Result<Self> {
        let url = format!("{relay_url}?role=server&token={auth_token}");
        let (ws, _resp) = tokio_tungstenite::connect_async(&url)
            .await
            .with_context(|| format!("connecting to relay at {relay_url}"))?;

        let (mut ws_write, mut ws_read) = ws.split();

        let (write_tx, mut write_rx) = mpsc::channel::<Message>(64);
        let (read_tx, read_rx) = mpsc::channel::<Message>(64);

        let task = tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(msg) = write_rx.recv() => {
                        if ws_write.send(msg).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(msg)) = ws_read.next() => {
                        if msg.is_close() {
                            break;
                        }
                        if read_tx.send(msg).await.is_err() {
                            break;
                        }
                    }
                    else => break,
                }
            }
        });

        Ok(Self {
            write_tx,
            read_rx: Arc::new(Mutex::new(read_rx)),
            _task: task,
        })
    }

    /// Send a JSON-RPC response back through the tunnel.
    pub async fn send_response(&self, resp: RpcResponse) -> Result<()> {
        let text = serde_json::to_string(&resp)?;
        self.write_tx
            .send(Message::Text(text.into()))
            .await
            .map_err(|_| anyhow::anyhow!("tunnel write channel closed"))?;
        Ok(())
    }

    /// Receive the next incoming JSON-RPC request.
    pub async fn recv_request(&self) -> Result<RpcRequest> {
        let mut rx = self.read_rx.lock().await;
        loop {
            let msg = rx
                .recv()
                .await
                .ok_or_else(|| anyhow::anyhow!("tunnel read channel closed"))?;

            if let Message::Text(text) = msg {
                let req: RpcRequest =
                    serde_json::from_str(&text).context("parsing JSON-RPC request")?;
                return Ok(req);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tunnel client (runs locally in SideX)
// ---------------------------------------------------------------------------

/// Client side of a remote tunnel.
///
/// Connects to the relay and is paired with the [`TunnelServer`] identified
/// by `tunnel_id`.
pub struct TunnelClient {
    write_tx: mpsc::Sender<Message>,
    read_rx: Arc<Mutex<mpsc::Receiver<Message>>>,
    next_id: Arc<Mutex<u64>>,
    _task: tokio::task::JoinHandle<()>,
}

impl TunnelClient {
    /// Connect to the relay and pair with the specified tunnel.
    pub async fn connect(relay_url: &str, tunnel_id: &str, auth_token: &str) -> Result<Self> {
        let url = format!("{relay_url}?role=client&tunnel={tunnel_id}&token={auth_token}");
        let (ws, _resp) = tokio_tungstenite::connect_async(&url)
            .await
            .with_context(|| format!("connecting to relay at {relay_url}"))?;

        let (mut ws_write, mut ws_read) = ws.split();
        let (write_tx, mut write_rx) = mpsc::channel::<Message>(64);
        let (read_tx, read_rx) = mpsc::channel::<Message>(64);

        let task = tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(msg) = write_rx.recv() => {
                        if ws_write.send(msg).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(msg)) = ws_read.next() => {
                        if msg.is_close() {
                            break;
                        }
                        if read_tx.send(msg).await.is_err() {
                            break;
                        }
                    }
                    else => break,
                }
            }
        });

        Ok(Self {
            write_tx,
            read_rx: Arc::new(Mutex::new(read_rx)),
            next_id: Arc::new(Mutex::new(1)),
            _task: task,
        })
    }

    /// Send a JSON-RPC request and wait for the matching response.
    pub async fn call(&self, method: &str, params: serde_json::Value) -> Result<serde_json::Value> {
        let id = {
            let mut next = self.next_id.lock().await;
            let id = *next;
            *next += 1;
            id
        };

        let req = RpcRequest::new(id, method, params);
        let text = serde_json::to_string(&req)?;
        self.write_tx
            .send(Message::Text(text.into()))
            .await
            .map_err(|_| anyhow::anyhow!("tunnel write channel closed"))?;

        let mut rx = self.read_rx.lock().await;
        loop {
            let msg = rx
                .recv()
                .await
                .ok_or_else(|| anyhow::anyhow!("tunnel read channel closed"))?;

            if let Message::Text(text) = msg {
                let resp: RpcResponse =
                    serde_json::from_str(&text).context("parsing JSON-RPC response")?;
                if resp.id == id {
                    if let Some(err) = resp.error {
                        bail!("RPC error {}: {}", err.code, err.message);
                    }
                    return Ok(resp.result.unwrap_or(serde_json::Value::Null));
                }
            }
        }
    }

    /// Send a fire-and-forget notification (no response expected).
    pub async fn notify(&self, method: &str, params: serde_json::Value) -> Result<()> {
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        let text = serde_json::to_string(&req)?;
        self.write_tx
            .send(Message::Text(text.into()))
            .await
            .map_err(|_| anyhow::anyhow!("tunnel write channel closed"))?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tunnel state & high-level connection
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TunnelState {
    Disconnected,
    Connecting,
    Connected { url: String },
    Error(String),
}

/// High-level tunnel connection with auto-reconnect and named access.
pub struct TunnelConnection {
    pub tunnel_id: String,
    pub state: TunnelState,
    pub name: String,
    pub access_token: Option<String>,
    server: Option<TunnelServer>,
}

impl TunnelConnection {
    /// Start a named tunnel server that accepts connections from remote clients.
    pub async fn start_tunnel(name: &str, relay_url: &str, auth_token: &str) -> Result<Self> {
        let server = TunnelServer::start(relay_url, auth_token).await?;
        let url = format!("{relay_url}/tunnel/{name}");
        Ok(Self {
            tunnel_id: name.to_string(),
            state: TunnelState::Connected { url: url.clone() },
            name: name.to_string(),
            access_token: Some(auth_token.to_string()),
            server: Some(server),
        })
    }

    /// Stop the tunnel and mark it disconnected.
    #[allow(clippy::unused_async)]
    pub async fn stop(&mut self) -> Result<()> {
        self.server = None;
        self.state = TunnelState::Disconnected;
        log::info!("tunnel '{}' stopped", self.name);
        Ok(())
    }

    /// Get the public URL for this tunnel.
    pub fn get_url(&self) -> Option<&str> {
        match &self.state {
            TunnelState::Connected { url } => Some(url.as_str()),
            _ => None,
        }
    }

    /// Attempt to reconnect with exponential backoff.
    pub async fn reconnect(&mut self, relay_url: &str, max_attempts: u32) -> Result<()> {
        let token = self
            .access_token
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("no access token for reconnect"))?
            .to_string();

        for attempt in 1..=max_attempts {
            self.state = TunnelState::Connecting;
            let delay = Duration::from_millis(500 * 2u64.pow(attempt.min(6)));
            tokio::time::sleep(delay).await;

            log::info!(
                "tunnel reconnect attempt {attempt}/{max_attempts} for '{}'",
                self.name
            );

            match TunnelServer::start(relay_url, &token).await {
                Ok(server) => {
                    let url = format!("{relay_url}/tunnel/{}", self.name);
                    self.server = Some(server);
                    self.state = TunnelState::Connected { url };
                    return Ok(());
                }
                Err(e) if attempt == max_attempts => {
                    self.state = TunnelState::Error(e.to_string());
                    return Err(e);
                }
                Err(e) => {
                    log::warn!("tunnel reconnect failed: {e}");
                }
            }
        }
        bail!("tunnel reconnection exhausted")
    }

    pub fn server(&self) -> Option<&TunnelServer> {
        self.server.as_ref()
    }
}
