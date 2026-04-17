//! VSIX package handling — parse, extract, validate, and install `.vsix` files.
//!
//! A `.vsix` is a ZIP archive following the Open VSIX Packaging format:
//!
//! ```text
//! [Content_Types].xml
//! extension.vsixmanifest          (XML metadata)
//! extension/
//!   package.json                  (extension manifest)
//!   README.md
//!   CHANGELOG.md
//!   LICENSE
//!   icon.png
//!   ...extension files...
//! ```

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::manifest::{parse_manifest_str, ExtensionManifest};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A fully unpacked VSIX package held in memory.
#[derive(Debug, Clone)]
pub struct VsixPackage {
    pub manifest: ExtensionManifest,
    pub readme: Option<String>,
    pub changelog: Option<String>,
    pub license_text: Option<String>,
    pub icon: Option<Vec<u8>>,
    /// Raw file contents keyed by their path relative to `extension/`.
    pub contents: HashMap<String, Vec<u8>>,
    pub vsix_manifest_xml: Option<String>,
}

/// An installed extension with its location on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledExtension {
    pub manifest: ExtensionManifest,
    pub install_dir: PathBuf,
}

/// Result of VSIX integrity validation.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

// ---------------------------------------------------------------------------
// Unpack
// ---------------------------------------------------------------------------

/// Unpacks a `.vsix` file into a `VsixPackage` in memory.
pub fn unpack_vsix(vsix_path: &Path) -> Result<VsixPackage> {
    let file = std::fs::File::open(vsix_path)
        .with_context(|| format!("failed to open .vsix: {}", vsix_path.display()))?;
    let mut archive = zip::ZipArchive::new(file).context("failed to read .vsix as ZIP")?;

    let mut manifest_json: Option<String> = None;
    let mut vsix_manifest_xml: Option<String> = None;
    let mut readme: Option<String> = None;
    let mut changelog: Option<String> = None;
    let mut license_text: Option<String> = None;
    let mut icon: Option<Vec<u8>> = None;
    let mut contents: HashMap<String, Vec<u8>> = HashMap::new();

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let Some(name) = entry.enclosed_name() else {
            continue;
        };
        let name_str = name.to_string_lossy().to_string();

        if name_str == "extension.vsixmanifest" {
            let mut buf = String::new();
            entry.read_to_string(&mut buf)?;
            vsix_manifest_xml = Some(buf);
            continue;
        }

        let Some(rel) = name_str.strip_prefix("extension/") else {
            continue;
        };
        if rel.is_empty() || entry.is_dir() {
            continue;
        }

        let mut buf = Vec::new();
        entry.read_to_end(&mut buf)?;

        let rel_lower = rel.to_lowercase();
        match rel_lower.as_str() {
            "package.json" => {
                manifest_json = Some(String::from_utf8_lossy(&buf).to_string());
            }
            "readme.md" => {
                readme = Some(String::from_utf8_lossy(&buf).to_string());
            }
            "changelog.md" => {
                changelog = Some(String::from_utf8_lossy(&buf).to_string());
            }
            "license" | "license.md" | "license.txt" => {
                license_text = Some(String::from_utf8_lossy(&buf).to_string());
            }
            _ => {}
        }

        if rel_lower.ends_with(".png")
            || rel_lower.ends_with(".jpg")
            || rel_lower.ends_with(".svg")
        {
            if rel_lower.contains("icon") {
                icon = Some(buf.clone());
            }
        }

        contents.insert(rel.to_string(), buf);
    }

    let json = manifest_json.context("missing extension/package.json in .vsix")?;
    let manifest = parse_manifest_str(&json)?;

    Ok(VsixPackage {
        manifest,
        readme,
        changelog,
        license_text,
        icon,
        contents,
        vsix_manifest_xml,
    })
}

// ---------------------------------------------------------------------------
// Validate
// ---------------------------------------------------------------------------

/// Validates a VSIX package for integrity and required fields.
pub fn validate_vsix(pkg: &VsixPackage) -> ValidationResult {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    if pkg.manifest.name.is_empty() {
        errors.push("manifest is missing 'name'".to_string());
    }
    if pkg.manifest.version.is_empty() {
        errors.push("manifest is missing 'version'".to_string());
    }

    if pkg.manifest.publisher.is_none() {
        warnings.push("manifest has no 'publisher' field".to_string());
    }
    if pkg.manifest.main.is_none() && pkg.manifest.browser.is_none() {
        warnings.push("no entry point: neither 'main' nor 'browser' specified".to_string());
    }
    if pkg.readme.is_none() {
        warnings.push("no README.md found".to_string());
    }
    if pkg.license_text.is_none() {
        warnings.push("no LICENSE file found".to_string());
    }
    if pkg.contents.is_empty() {
        errors.push("VSIX contains no extension files".to_string());
    }

    if let Some(ref main) = pkg.manifest.main {
        let main_normalized = main.strip_prefix("./").unwrap_or(main);
        if !pkg.contents.contains_key(main_normalized) {
            let with_js = format!("{main_normalized}.js");
            if !pkg.contents.contains_key(&with_js) {
                warnings.push(format!("entry point '{main}' not found in archive"));
            }
        }
    }

    ValidationResult {
        valid: errors.is_empty(),
        errors,
        warnings,
    }
}

/// Validates a `.vsix` file on disk.
pub fn validate_vsix_file(vsix_path: &Path) -> Result<ValidationResult> {
    let pkg = unpack_vsix(vsix_path)?;
    Ok(validate_vsix(&pkg))
}

// ---------------------------------------------------------------------------
// Install
// ---------------------------------------------------------------------------

/// Installs a VSIX package (already unpacked) to an extensions directory.
pub fn install_package(pkg: &VsixPackage, extensions_dir: &Path) -> Result<InstalledExtension> {
    let ext_id = pkg.manifest.canonical_id();
    let ext_dir = extensions_dir.join(&ext_id);

    if ext_dir.exists() {
        std::fs::remove_dir_all(&ext_dir).with_context(|| {
            format!("failed to remove existing extension dir: {}", ext_dir.display())
        })?;
    }
    std::fs::create_dir_all(&ext_dir)?;

    for (rel_path, data) in &pkg.contents {
        let out_path = ext_dir.join(rel_path);
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&out_path, data)?;
    }

    Ok(InstalledExtension {
        manifest: pkg.manifest.clone(),
        install_dir: ext_dir,
    })
}

/// Convenience: unpack, validate, and install a `.vsix` file.
pub fn install_vsix(vsix_path: &Path, extensions_dir: &Path) -> Result<InstalledExtension> {
    let pkg = unpack_vsix(vsix_path)?;
    let validation = validate_vsix(&pkg);
    if !validation.valid {
        anyhow::bail!(
            "VSIX validation failed: {}",
            validation.errors.join("; ")
        );
    }
    install_package(&pkg, extensions_dir)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_catches_empty_name() {
        let pkg = VsixPackage {
            manifest: ExtensionManifest {
                name: String::new(),
                version: "1.0.0".into(),
                ..default_test_manifest()
            },
            readme: None,
            changelog: None,
            license_text: None,
            icon: None,
            contents: HashMap::from([("dist/main.js".into(), vec![])]),
            vsix_manifest_xml: None,
        };
        let r = validate_vsix(&pkg);
        assert!(!r.valid);
        assert!(r.errors.iter().any(|e| e.contains("name")));
    }

    #[test]
    fn validation_warns_on_no_readme() {
        let pkg = VsixPackage {
            manifest: default_test_manifest(),
            readme: None,
            changelog: None,
            license_text: Some("MIT".into()),
            icon: None,
            contents: HashMap::from([("dist/main.js".into(), vec![])]),
            vsix_manifest_xml: None,
        };
        let r = validate_vsix(&pkg);
        assert!(r.valid);
        assert!(r.warnings.iter().any(|w| w.contains("README")));
    }

    #[test]
    fn validation_ok_for_complete_package() {
        let pkg = VsixPackage {
            manifest: default_test_manifest(),
            readme: Some("# Test".into()),
            changelog: Some("## 1.0.0".into()),
            license_text: Some("MIT".into()),
            icon: None,
            contents: HashMap::from([
                ("package.json".into(), b"{}".to_vec()),
                ("dist/main.js".into(), b"module.exports={}".to_vec()),
            ]),
            vsix_manifest_xml: None,
        };
        let r = validate_vsix(&pkg);
        assert!(r.valid);
        assert!(r.errors.is_empty());
    }

    fn default_test_manifest() -> ExtensionManifest {
        ExtensionManifest {
            id: "test.ext".into(),
            name: "ext".into(),
            display_name: "Test Extension".into(),
            version: "1.0.0".into(),
            publisher: Some("test".into()),
            main: Some("./dist/main.js".into()),
            ..serde_json::from_str(
                r#"{"name":"ext","version":"1.0.0","publisher":"test","main":"./dist/main.js"}"#,
            )
            .unwrap()
        }
    }
}
