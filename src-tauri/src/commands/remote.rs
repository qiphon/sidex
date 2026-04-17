use std::path::PathBuf;

use serde::Serialize;

// ── Response structs ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct SshHostInfo {
    pub host: String,
    pub hostname: Option<String>,
    pub port: Option<u16>,
    pub user: Option<String>,
    pub identity_file: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WslDistroInfo {
    pub name: String,
    pub is_default: bool,
    pub version: u8,
    pub state: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContainerListEntry {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: String,
    pub ports: String,
}

// ── Commands ────────────────────────────────────────────────────────

#[tauri::command]
pub fn remote_list_ssh_hosts() -> Result<Vec<SshHostInfo>, String> {
    let config_path = dirs::home_dir()
        .map(|h| h.join(".ssh/config"))
        .unwrap_or_else(|| PathBuf::from("~/.ssh/config"));

    if !config_path.exists() {
        return Ok(Vec::new());
    }

    let hosts = sidex_remote::ssh::parse_ssh_config(&config_path)
        .map_err(|e| format!("failed to parse SSH config: {e}"))?;

    Ok(hosts
        .into_iter()
        .filter(|h| !h.host_pattern.contains('*'))
        .map(|h| SshHostInfo {
            host: h.host_pattern,
            hostname: h.hostname,
            port: h.port,
            user: h.user,
            identity_file: h.identity_file.map(|p| p.to_string_lossy().into_owned()),
        })
        .collect())
}

#[tauri::command]
pub fn remote_list_wsl_distros() -> Result<Vec<WslDistroInfo>, String> {
    #[cfg(target_os = "windows")]
    {
        let distros = sidex_remote::wsl::list_distributions()
            .map_err(|e| format!("WSL list failed: {e}"))?;
        Ok(distros
            .into_iter()
            .map(|d| WslDistroInfo {
                name: d.name,
                is_default: d.is_default,
                version: d.version,
                state: d.state,
            })
            .collect())
    }

    #[cfg(not(target_os = "windows"))]
    {
        Ok(Vec::new())
    }
}

#[tauri::command]
pub async fn remote_list_containers() -> Result<Vec<ContainerListEntry>, String> {
    let output = tokio::process::Command::new("docker")
        .args([
            "ps",
            "-a",
            "--format",
            "{{.ID}}\t{{.Names}}\t{{.Image}}\t{{.Status}}\t{{.Ports}}",
        ])
        .output()
        .await
        .map_err(|e| format!("failed to run docker ps: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("Cannot connect") || stderr.contains("not found") {
            return Ok(Vec::new());
        }
        return Err(format!("docker ps failed: {stderr}"));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout
        .lines()
        .filter(|l| !l.is_empty())
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(5, '\t').collect();
            if parts.len() >= 4 {
                Some(ContainerListEntry {
                    id: parts[0].to_string(),
                    name: parts[1].to_string(),
                    image: parts[2].to_string(),
                    status: parts[3].to_string(),
                    ports: parts.get(4).unwrap_or(&"").to_string(),
                })
            } else {
                None
            }
        })
        .collect())
}
