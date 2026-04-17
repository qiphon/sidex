//! Product configuration — application-level metadata analogous to VS Code's
//! `product.json`.
//!
//! [`ProductConfig`] describes the application name, version, marketplace URL,
//! update endpoint, build quality, and other compile-time or runtime-overridable
//! values.

use serde::{Deserialize, Serialize};

/// Application-level product configuration.
///
/// In a release build these values come from the embedded defaults; they can
/// be overridden by placing a `product.json` next to the binary.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductConfig {
    /// Short product name (e.g. `"SideX"`).
    pub name: String,
    /// SemVer version (e.g. `"0.2.0"`).
    pub version: String,
    /// Long product name (e.g. `"SideX - Code Editor"`).
    pub name_long: String,
    /// Binary / application identifier (used for data dirs, IPC, etc.).
    pub application_name: String,
    /// Dot-directory name for user data (e.g. `".sidex"`).
    pub data_folder_name: String,
    /// Custom URL protocol (e.g. `"sidex://"`).
    pub url_protocol: String,
    /// Extension marketplace API endpoint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extension_gallery_url: Option<String>,
    /// Auto-update endpoint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update_url: Option<String>,
    /// Git commit hash at build time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    /// ISO-8601 build date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    /// Release quality channel (`"stable"` or `"insiders"`).
    pub quality: String,
    /// License identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license_url: Option<String>,
    /// Bug report / issue tracker URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_issue_url: Option<String>,
    /// Documentation URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub documentation_url: Option<String>,
}

impl ProductConfig {
    /// Load product configuration.
    ///
    /// Checks for a `product.json` file next to the running binary first.
    /// Falls back to compiled-in defaults if the file is absent or
    /// unparseable.
    pub fn load() -> Self {
        if let Some(path) = exe_sibling("product.json") {
            if let Ok(contents) = std::fs::read_to_string(&path) {
                if let Ok(cfg) = serde_json::from_str::<Self>(&contents) {
                    log::info!("loaded product config from {}", path.display());
                    return cfg;
                }
                log::warn!("malformed product.json at {}, using defaults", path.display());
            }
        }
        Self::default()
    }

    /// A one-line version string suitable for `--version` output.
    pub fn version_string(&self) -> String {
        let mut s = format!("{} {}", self.name, self.version);
        if let Some(ref commit) = self.commit {
            let short = if commit.len() >= 7 { &commit[..7] } else { commit };
            s.push_str(&format!(" ({short})"));
        }
        s
    }

    /// The user-data directory path (e.g. `~/.sidex`).
    pub fn data_dir(&self) -> Option<std::path::PathBuf> {
        dirs::home_dir().map(|h| h.join(&self.data_folder_name))
    }
}

impl Default for ProductConfig {
    fn default() -> Self {
        Self {
            name: "SideX".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            name_long: "SideX - Code Editor".into(),
            application_name: "sidex".into(),
            data_folder_name: ".sidex".into(),
            url_protocol: "sidex".into(),
            extension_gallery_url: None,
            update_url: None,
            commit: option_env!("SIDEX_COMMIT").map(Into::into),
            date: option_env!("SIDEX_BUILD_DATE").map(Into::into),
            quality: "stable".into(),
            license_url: Some("https://github.com/sidenai/sidex/blob/main/LICENSE".into()),
            report_issue_url: Some("https://github.com/sidenai/sidex/issues".into()),
            documentation_url: Some("https://github.com/sidenai/sidex/wiki".into()),
        }
    }
}

/// Resolve a filename next to the current executable.
fn exe_sibling(name: &str) -> Option<std::path::PathBuf> {
    std::env::current_exe().ok().and_then(|exe| {
        exe.parent().map(|dir| dir.join(name))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_sane() {
        let cfg = ProductConfig::default();
        assert_eq!(cfg.name, "SideX");
        assert_eq!(cfg.application_name, "sidex");
        assert_eq!(cfg.data_folder_name, ".sidex");
        assert_eq!(cfg.quality, "stable");
        assert!(!cfg.version.is_empty());
    }

    #[test]
    fn version_string_without_commit() {
        let cfg = ProductConfig::default();
        let v = cfg.version_string();
        assert!(v.starts_with("SideX "));
    }

    #[test]
    fn version_string_with_commit() {
        let mut cfg = ProductConfig::default();
        cfg.commit = Some("abc1234def5678".into());
        let v = cfg.version_string();
        assert!(v.contains("(abc1234)"));
    }

    #[test]
    fn roundtrip_json() {
        let cfg = ProductConfig::default();
        let json = serde_json::to_string_pretty(&cfg).unwrap();
        let parsed: ProductConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "SideX");
        assert_eq!(parsed.quality, "stable");
    }

    #[test]
    fn load_falls_back_to_default() {
        let cfg = ProductConfig::load();
        assert_eq!(cfg.name, "SideX");
    }

    #[test]
    fn data_dir_ends_with_sidex() {
        let cfg = ProductConfig::default();
        if let Some(dir) = cfg.data_dir() {
            assert!(dir.ends_with(".sidex"));
        }
    }
}
