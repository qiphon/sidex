//! GitHub Codespaces remote transport backend.
//!
//! Manages Codespace lifecycle via the GitHub REST API and connects to
//! running instances over SSH (GitHub provides SSH access to Codespaces).

use std::path::Path;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::ssh::{SshAuth, SshTransport};
use crate::transport::{DirEntry, ExecOutput, FileStat, RemotePty, RemoteTransport};

// ---------------------------------------------------------------------------
// API base
// ---------------------------------------------------------------------------

const GITHUB_API: &str = "https://api.github.com";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Metadata for a single GitHub Codespace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Codespace {
    pub name: String,
    pub state: String,
    #[serde(default)]
    pub machine_type: Option<String>,
    #[serde(default)]
    pub repository: String,
    #[serde(default)]
    pub branch: String,
    #[serde(default)]
    pub created_at: String,
}

/// Response wrapper for listing codespaces.
#[derive(Deserialize)]
struct ListResponse {
    codespaces: Vec<Codespace>,
}

/// Lifecycle state of a codespace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CodespaceState {
    Available,
    Starting,
    Running,
    Stopping,
    Stopped,
    Rebuilding,
    Deleted,
}

impl CodespaceState {
    pub fn from_api(s: &str) -> Self {
        match s {
            "Available" => Self::Available,
            "Starting" => Self::Starting,
            "Running" => Self::Running,
            "Stopping" => Self::Stopping,
            "Rebuilding" => Self::Rebuilding,
            "Deleted" => Self::Deleted,
            _ => Self::Stopped,
        }
    }
}

/// Extended codespace information with timing and retention.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodespaceInfo {
    pub name: String,
    pub display_name: String,
    pub repository: String,
    pub branch: String,
    pub machine_type: String,
    pub state: CodespaceState,
    pub created_at: String,
    pub last_used: String,
    pub idle_timeout_minutes: u64,
    pub retention_period_minutes: u64,
}

impl From<&Codespace> for CodespaceInfo {
    fn from(cs: &Codespace) -> Self {
        Self {
            name: cs.name.clone(),
            display_name: cs.name.clone(),
            repository: cs.repository.clone(),
            branch: cs.branch.clone(),
            machine_type: cs.machine_type.clone().unwrap_or_default(),
            state: CodespaceState::from_api(&cs.state),
            created_at: cs.created_at.clone(),
            last_used: String::new(),
            idle_timeout_minutes: 30,
            retention_period_minutes: 43200,
        }
    }
}

/// High-level Codespaces connection wrapping transport and metadata.
pub struct CodespacesConnection {
    pub codespace: CodespaceInfo,
    pub token: String,
    transport: CodespacesTransport,
}

impl CodespacesConnection {
    pub async fn connect(name: &str, token: &str) -> Result<Self> {
        let transport = CodespacesTransport::connect(name, token).await?;
        let codespaces = list_codespaces(token).await?;
        let cs = codespaces
            .iter()
            .find(|c| c.name == name)
            .ok_or_else(|| anyhow::anyhow!("codespace not found"))?;

        Ok(Self {
            codespace: CodespaceInfo::from(cs),
            token: token.to_string(),
            transport,
        })
    }

    pub fn transport(&self) -> &CodespacesTransport {
        &self.transport
    }
}

/// List codespaces with rich info.
pub async fn list_codespace_info(token: &str) -> Result<Vec<CodespaceInfo>> {
    let codespaces = list_codespaces(token).await?;
    Ok(codespaces.iter().map(CodespaceInfo::from).collect())
}

/// Wait for a codespace to reach the Available state, polling periodically.
pub async fn wait_for_codespace(name: &str, token: &str, timeout: Duration) -> Result<Codespace> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let codespaces = list_codespaces(token).await?;
        if let Some(cs) = codespaces.into_iter().find(|c| c.name == name) {
            if cs.state == "Available" {
                return Ok(cs);
            }
        }
        if tokio::time::Instant::now() >= deadline {
            bail!("timed out waiting for codespace '{name}' to become Available");
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

/// Rebuild a codespace.
pub async fn rebuild_codespace(name: &str, token: &str) -> Result<()> {
    let client = gh_client(token)?;
    let resp = client
        .post(format!("{GITHUB_API}/user/codespaces/{name}/rebuild"))
        .send()
        .await
        .context("rebuilding codespace")?;
    if !resp.status().is_success() {
        bail!("rebuild codespace: {}", resp.text().await?);
    }
    Ok(())
}

/// Get available machine types for a repository.
pub async fn list_machine_types(token: &str, repo: &str) -> Result<Vec<String>> {
    #[derive(Deserialize)]
    struct Machine {
        name: String,
    }
    #[derive(Deserialize)]
    struct MachinesResp {
        machines: Vec<Machine>,
    }
    let client = gh_client(token)?;
    let resp = client
        .get(format!("{GITHUB_API}/repos/{repo}/codespaces/machines"))
        .send()
        .await
        .context("listing machine types")?;
    if !resp.status().is_success() {
        bail!("list machines: {}", resp.text().await?);
    }
    let body: MachinesResp = resp.json().await?;
    Ok(body.machines.into_iter().map(|m| m.name).collect())
}

// ---------------------------------------------------------------------------
// Codespace lifecycle (API)
// ---------------------------------------------------------------------------

fn gh_client(token: &str) -> Result<reqwest::Client> {
    use reqwest::header;
    let mut headers = header::HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        header::HeaderValue::from_str(&format!("Bearer {token}"))?,
    );
    headers.insert(
        header::ACCEPT,
        header::HeaderValue::from_static("application/vnd.github+json"),
    );
    headers.insert(
        "X-GitHub-Api-Version",
        header::HeaderValue::from_static("2022-11-28"),
    );
    headers.insert(
        header::USER_AGENT,
        header::HeaderValue::from_static("sidex"),
    );
    Ok(reqwest::Client::builder()
        .default_headers(headers)
        .build()?)
}

/// List all codespaces for the authenticated user.
pub async fn list_codespaces(token: &str) -> Result<Vec<Codespace>> {
    let client = gh_client(token)?;
    let resp = client
        .get(format!("{GITHUB_API}/user/codespaces"))
        .send()
        .await
        .context("listing codespaces")?;

    if !resp.status().is_success() {
        bail!("GitHub API error {}: {}", resp.status(), resp.text().await?);
    }

    let body: ListResponse = resp.json().await?;
    Ok(body.codespaces)
}

/// Create a new codespace for a given repository.
pub async fn create_codespace(
    repo: &str,
    branch: &str,
    machine: &str,
    token: &str,
) -> Result<Codespace> {
    let client = gh_client(token)?;
    let body = serde_json::json!({
        "repository_id": repo,
        "ref": branch,
        "machine": machine,
    });

    let resp = client
        .post(format!("{GITHUB_API}/user/codespaces"))
        .json(&body)
        .send()
        .await
        .context("creating codespace")?;

    if !resp.status().is_success() {
        bail!("GitHub API error {}: {}", resp.status(), resp.text().await?);
    }

    let cs: Codespace = resp.json().await?;
    Ok(cs)
}

/// Start a stopped codespace.
pub async fn start_codespace(name: &str, token: &str) -> Result<()> {
    let client = gh_client(token)?;
    let resp = client
        .post(format!("{GITHUB_API}/user/codespaces/{name}/start"))
        .send()
        .await?;

    if !resp.status().is_success() {
        bail!("start codespace: {}", resp.text().await?);
    }
    Ok(())
}

/// Stop a running codespace.
pub async fn stop_codespace(name: &str, token: &str) -> Result<()> {
    let client = gh_client(token)?;
    let resp = client
        .post(format!("{GITHUB_API}/user/codespaces/{name}/stop"))
        .send()
        .await?;

    if !resp.status().is_success() {
        bail!("stop codespace: {}", resp.text().await?);
    }
    Ok(())
}

/// Delete a codespace.
pub async fn delete_codespace(name: &str, token: &str) -> Result<()> {
    let client = gh_client(token)?;
    let resp = client
        .delete(format!("{GITHUB_API}/user/codespaces/{name}"))
        .send()
        .await?;

    if !resp.status().is_success() {
        bail!("delete codespace: {}", resp.text().await?);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Transport
// ---------------------------------------------------------------------------

/// GitHub Codespaces [`RemoteTransport`].
///
/// Internally delegates to an [`SshTransport`] once the codespace is running,
/// since GitHub exposes SSH access to codespaces via `gh cs ssh`.
#[allow(dead_code)]
pub struct CodespacesTransport {
    name: String,
    token: String,
    ssh: SshTransport,
}

impl CodespacesTransport {
    /// Connect to an already-running codespace by name.
    ///
    /// The codespace must be in the "Available" state.  This opens an SSH
    /// connection using the GitHub SSH relay (`codespaces.githubusercontent.com`).
    pub async fn connect(codespace_name: &str, token: &str) -> Result<Self> {
        let codespaces = list_codespaces(token).await?;
        let cs = codespaces
            .iter()
            .find(|c| c.name == codespace_name)
            .ok_or_else(|| anyhow::anyhow!("codespace '{codespace_name}' not found"))?;

        if cs.state != "Available" {
            bail!(
                "codespace '{}' is in state '{}', not Available",
                codespace_name,
                cs.state
            );
        }

        let ssh_host = format!("{codespace_name}.codespaces.githubusercontent.com");
        let ssh = SshTransport::connect(&ssh_host, 22, SshAuth::Agent)
            .await
            .context("SSH to codespace")?;

        Ok(Self {
            name: codespace_name.to_string(),
            token: token.to_string(),
            ssh,
        })
    }
}

#[async_trait::async_trait]
impl RemoteTransport for CodespacesTransport {
    async fn exec(&self, command: &str) -> Result<ExecOutput> {
        self.ssh.exec(command).await
    }

    async fn read_file(&self, path: &str) -> Result<Vec<u8>> {
        self.ssh.read_file(path).await
    }

    async fn write_file(&self, path: &str, data: &[u8]) -> Result<()> {
        self.ssh.write_file(path, data).await
    }

    async fn read_dir(&self, path: &str) -> Result<Vec<DirEntry>> {
        self.ssh.read_dir(path).await
    }

    async fn stat(&self, path: &str) -> Result<FileStat> {
        self.ssh.stat(path).await
    }

    async fn open_pty(&self, cols: u16, rows: u16) -> Result<RemotePty> {
        self.ssh.open_pty(cols, rows).await
    }

    async fn upload(&self, local: &Path, remote: &str) -> Result<()> {
        self.ssh.upload(local, remote).await
    }

    async fn download(&self, remote: &str, local: &Path) -> Result<()> {
        self.ssh.download(remote, local).await
    }

    async fn disconnect(&self) -> Result<()> {
        self.ssh.disconnect().await
    }
}
