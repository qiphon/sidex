use crate::commands::extension_platform::{
    read_extension_manifest, read_vsix_manifest, sanitize_ext_id, user_extensions_dir,
    ExtensionManifest,
};
use serde::Serialize;
use sidex_extensions::marketplace::MarketplaceClient;
use sidex_extensions::contributions::{parse_contributions, ContributionPoint};
use std::fs::{self, File};
use std::io::{Cursor, Read};
use std::path::Path;
use tauri::AppHandle;

#[derive(Debug, Serialize)]
pub struct InstalledExtension {
    pub id: String,
    pub name: String,
    pub version: String,
    pub path: String,
}

#[tauri::command]
pub async fn install_extension(vsix_path: String) -> Result<InstalledExtension, String> {
    let vsix = Path::new(&vsix_path);
    if !vsix.exists() {
        return Err(format!("VSIX not found: {vsix_path}"));
    }

    let file = File::open(vsix).map_err(|e| format!("open: {e}"))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("bad vsix: {e}"))?;

    let manifest = read_vsix_manifest(&mut archive)?;

    let safe_id = sanitize_ext_id(&manifest.id)?;
    let ext_dir = user_extensions_dir().join(&safe_id);
    if ext_dir.exists() {
        fs::remove_dir_all(&ext_dir).map_err(|e| format!("cleanup: {e}"))?;
    }
    fs::create_dir_all(&ext_dir).map_err(|e| format!("mkdir: {e}"))?;

    let prefix = "extension/";
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| format!("entry: {e}"))?;
        let raw_name = entry.name().to_string();

        if !raw_name.starts_with(prefix) {
            continue;
        }

        let rel = &raw_name[prefix.len()..];
        if rel.is_empty() || rel.contains("..") {
            continue;
        }

        let target = ext_dir.join(rel);

        if entry.is_dir() {
            fs::create_dir_all(&target).map_err(|e| format!("mkdir {rel}: {e}"))?;
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).ok();
            }
            #[allow(clippy::cast_possible_truncation)]
            let mut buf = Vec::with_capacity(entry.size() as usize);
            entry
                .read_to_end(&mut buf)
                .map_err(|e| format!("read {rel}: {e}"))?;
            fs::write(&target, &buf).map_err(|e| format!("write {rel}: {e}"))?;
            #[cfg(unix)]
            if entry.unix_mode().is_some_and(|m| m & 0o111 != 0) || rel.starts_with("bin/") {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&target, fs::Permissions::from_mode(0o755)).ok();
            }
        }
    }

    log::info!("installed extension {} to {}", safe_id, ext_dir.display());

    Ok(InstalledExtension {
        id: safe_id,
        name: manifest.name,
        version: manifest.version,
        path: ext_dir.to_string_lossy().to_string(),
    })
}

fn extract_vsix_bytes(data: &[u8]) -> Result<InstalledExtension, String> {
    let cursor = Cursor::new(data);
    let mut archive = zip::ZipArchive::new(cursor).map_err(|e| format!("bad vsix: {e}"))?;
    let manifest = read_vsix_manifest(&mut archive)?;
    let safe_id = sanitize_ext_id(&manifest.id)?;
    let ext_dir = user_extensions_dir().join(&safe_id);
    if ext_dir.exists() {
        fs::remove_dir_all(&ext_dir).map_err(|e| format!("cleanup: {e}"))?;
    }
    fs::create_dir_all(&ext_dir).map_err(|e| format!("mkdir: {e}"))?;
    let prefix = "extension/";
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| format!("entry: {e}"))?;
        let raw_name = entry.name().to_string();
        if !raw_name.starts_with(prefix) {
            continue;
        }
        let rel = &raw_name[prefix.len()..];
        if rel.is_empty() || rel.contains("..") {
            continue;
        }
        let target = ext_dir.join(rel);
        if entry.is_dir() {
            fs::create_dir_all(&target).map_err(|e| format!("mkdir {rel}: {e}"))?;
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).ok();
            }
            #[allow(clippy::cast_possible_truncation)]
            let mut buf = Vec::with_capacity(entry.size() as usize);
            entry
                .read_to_end(&mut buf)
                .map_err(|e| format!("read {rel}: {e}"))?;
            fs::write(&target, &buf).map_err(|e| format!("write {rel}: {e}"))?;
            #[cfg(unix)]
            if entry.unix_mode().is_some_and(|m| m & 0o111 != 0) || rel.starts_with("bin/") {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&target, fs::Permissions::from_mode(0o755)).ok();
            }
        }
    }
    log::info!("installed extension {} to {}", safe_id, ext_dir.display());
    Ok(InstalledExtension {
        id: safe_id,
        name: manifest.name,
        version: manifest.version,
        path: ext_dir.to_string_lossy().to_string(),
    })
}

#[tauri::command]
pub async fn install_extension_from_url(url: String) -> Result<InstalledExtension, String> {
    log::info!("downloading extension from {url}");
    let resp = reqwest::get(&url)
        .await
        .map_err(|e| format!("download: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("download failed: HTTP {}", resp.status()));
    }
    let bytes = resp.bytes().await.map_err(|e| format!("read body: {e}"))?;
    extract_vsix_bytes(&bytes)
}

#[tauri::command]
pub async fn uninstall_extension(extension_id: String) -> Result<(), String> {
    let safe_id = sanitize_ext_id(&extension_id)?;
    let ext_dir = user_extensions_dir().join(&safe_id);
    if !ext_dir.exists() {
        return Err(format!("not installed: {extension_id}"));
    }
    fs::remove_dir_all(&ext_dir).map_err(|e| format!("remove: {e}"))?;
    log::info!("uninstalled {extension_id}");
    Ok(())
}

#[tauri::command]
pub async fn list_installed_extensions(app: AppHandle) -> Result<Vec<InstalledExtension>, String> {
    let dir = user_extensions_dir();
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut out = Vec::new();
    let entries = fs::read_dir(&dir).map_err(|e| format!("readdir: {e}"))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if let Ok(ExtensionManifest {
            id,
            display_name,
            version,
            path,
            ..
        }) = read_extension_manifest(&app, &path)
        {
            out.push(InstalledExtension {
                id,
                name: display_name,
                version,
                path,
            });
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Marketplace search
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct MarketplaceResult {
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub version: String,
    pub publisher: String,
    pub install_count: u64,
    pub rating: f32,
    pub icon_url: Option<String>,
    pub download_url: String,
}

#[tauri::command]
pub async fn extension_search_marketplace(
    query: String,
    page: u32,
) -> Result<Vec<MarketplaceResult>, String> {
    let mut client = MarketplaceClient::new();
    let result = client
        .search(&query, page, 20)
        .await
        .map_err(|e| format!("marketplace search: {e}"))?;

    Ok(result
        .results
        .into_iter()
        .map(|ext| {
            let desc = if ext.short_description.is_empty() {
                ext.description.clone()
            } else {
                ext.short_description.clone()
            };
            MarketplaceResult {
                id: ext.id,
                name: ext.name,
                display_name: ext.display_name,
                description: desc,
                version: ext.version,
                publisher: ext.publisher.display_name,
                install_count: ext.install_count,
                rating: ext.rating,
                icon_url: ext.icon_url,
                download_url: ext.download_url,
            }
        })
        .collect())
}

// ---------------------------------------------------------------------------
// Extension contributions
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct ContributionInfo {
    pub kind: String,
    pub count: usize,
    pub details: Vec<String>,
}

fn summarize_point(point: &ContributionPoint) -> ContributionInfo {
    match point {
        ContributionPoint::Commands(v) => ContributionInfo {
            kind: "commands".into(),
            count: v.len(),
            details: v.iter().map(|c| c.title.clone()).collect(),
        },
        ContributionPoint::Languages(v) => ContributionInfo {
            kind: "languages".into(),
            count: v.len(),
            details: v.iter().map(|l| l.id.clone()).collect(),
        },
        ContributionPoint::Themes(v) => ContributionInfo {
            kind: "themes".into(),
            count: v.len(),
            details: v.iter().map(|t| t.label.clone()).collect(),
        },
        ContributionPoint::Grammars(v) => ContributionInfo {
            kind: "grammars".into(),
            count: v.len(),
            details: v.iter().map(|g| g.scope_name.clone()).collect(),
        },
        ContributionPoint::Keybindings(v) => ContributionInfo {
            kind: "keybindings".into(),
            count: v.len(),
            details: v.iter().map(|k| k.command.clone()).collect(),
        },
        ContributionPoint::Snippets(v) => ContributionInfo {
            kind: "snippets".into(),
            count: v.len(),
            details: v.iter().map(|s| s.path.clone()).collect(),
        },
        ContributionPoint::Debuggers(v) => ContributionInfo {
            kind: "debuggers".into(),
            count: v.len(),
            details: v.iter().map(|d| d.label.clone()).collect(),
        },
        ContributionPoint::Views(m) => ContributionInfo {
            kind: "views".into(),
            count: m.values().map(Vec::len).sum(),
            details: m.values().flatten().map(|v| v.id.clone()).collect(),
        },
        ContributionPoint::Configuration(v) => ContributionInfo {
            kind: "configuration".into(),
            count: v.len(),
            details: v.iter().filter_map(|c| c.title.clone()).collect(),
        },
        ContributionPoint::IconThemes(v) => ContributionInfo {
            kind: "iconThemes".into(),
            count: v.len(),
            details: v.iter().map(|t| t.label.clone()).collect(),
        },
        ContributionPoint::ViewsContainers(m) => ContributionInfo {
            kind: "viewsContainers".into(),
            count: m.values().map(Vec::len).sum(),
            details: m.values().flatten().map(|c| c.title.clone()).collect(),
        },
        ContributionPoint::Menus(m) => ContributionInfo {
            kind: "menus".into(),
            count: m.values().map(Vec::len).sum(),
            details: m.keys().cloned().collect(),
        },
        ContributionPoint::TaskDefinitions(v) => ContributionInfo {
            kind: "taskDefinitions".into(),
            count: v.len(),
            details: v.iter().map(|t| t.task_type.clone()).collect(),
        },
        ContributionPoint::ProblemMatchers(v) => ContributionInfo {
            kind: "problemMatchers".into(),
            count: v.len(),
            details: v.iter().map(|p| p.name.clone()).collect(),
        },
        ContributionPoint::Terminal(t) => ContributionInfo {
            kind: "terminal".into(),
            count: t.profiles.len(),
            details: t.profiles.iter().map(|p| p.title.clone()).collect(),
        },
    }
}

#[tauri::command]
pub async fn extension_get_contributions(
    extension_dir: String,
) -> Result<Vec<ContributionInfo>, String> {
    let pkg_path = Path::new(&extension_dir).join("package.json");
    let raw = fs::read_to_string(&pkg_path)
        .map_err(|e| format!("read package.json: {e}"))?;
    let value: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("parse package.json: {e}"))?;

    let points = parse_contributions(&value);
    Ok(points.iter().map(summarize_point).collect())
}
