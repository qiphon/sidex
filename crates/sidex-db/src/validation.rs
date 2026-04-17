//! Security-critical input validation.
//!
//! Ported from `src-tauri/src/commands/validation.rs`.  These helpers reject
//! dangerous inputs **before** they reach the filesystem or shell layer.

use std::path::Path;

/// Rejects empty paths, NUL bytes, and parent-directory traversal (`..`).
pub fn validate_path(path: &str) -> Result<(), String> {
    if path.is_empty() {
        return Err("path must not be empty".to_string());
    }
    if path.contains('\0') {
        return Err("path must not contain NUL bytes".to_string());
    }
    for comp in Path::new(path).components() {
        if let std::path::Component::ParentDir = comp {
            return Err("path must not contain parent directory references (..)".to_string());
        }
    }
    Ok(())
}

/// Rejects arguments containing NUL bytes.
pub fn validate_args(args: &[&str]) -> Result<(), String> {
    for (i, arg) in args.iter().enumerate() {
        if arg.contains('\0') {
            return Err(format!("argument at index {i} must not contain NUL bytes"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_rejects_empty() {
        assert!(validate_path("").is_err());
    }

    #[test]
    fn path_rejects_nul() {
        assert!(validate_path("file.txt\0").is_err());
    }

    #[test]
    fn path_rejects_traversal() {
        assert!(validate_path("../../../etc/passwd").is_err());
        assert!(validate_path("folder/../sensitive").is_err());
        assert!(validate_path("/home/user/../root").is_err());
    }

    #[test]
    fn path_accepts_valid() {
        assert!(validate_path("/home/user/file.txt").is_ok());
        assert!(validate_path("relative/path").is_ok());
        assert!(validate_path(".").is_ok());
    }

    #[test]
    fn args_rejects_nul() {
        assert!(validate_args(&["arg\0"]).is_err());
    }

    #[test]
    fn args_accepts_valid() {
        assert!(validate_args(&["--flag", "value"]).is_ok());
        assert!(validate_args(&[]).is_ok());
        assert!(validate_args(&["--allow-empty-message", "-m", ""]).is_ok());
    }
}
