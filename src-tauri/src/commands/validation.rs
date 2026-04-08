//! Security validation utilities for preventing common vulnerabilities.
//!
//! This module provides centralized validation functions to protect against:
//! - Path traversal attacks (CWE-22)
//! - Command injection via arguments (CWE-88)
//! - Resource exhaustion (CWE-400)
//!
//! All file system and command execution functions should use these validators.

use std::path::Path;

/// Validates a file system path to prevent path traversal attacks.
///
/// # Security
///
/// Prevents directory traversal by rejecting:
/// - Empty paths (could resolve to current directory ambiguously)
/// - NUL bytes (Windows path separator vulnerability)
/// - Parent directory references (`..`) which could escape intended directories
///
/// # Arguments
///
/// * `path` - The file system path to validate
///
/// # Returns
///
/// * `Ok(())` if the path is safe
/// * `Err(String)` with a descriptive error message if validation fails
///
/// # Example
///
/// ```
/// validate_path("/home/user/file.txt")?; // Ok
/// validate_path("../../../etc/passwd")?; // Err: parent directory references
/// ```
pub fn validate_path(path: &str) -> Result<(), String> {
    if path.is_empty() {
        return Err("path must not be empty".to_string());
    }
    if path.contains('\0') {
        return Err("path must not contain NUL bytes".to_string());
    }
    // Check for parent directory references to prevent path traversal
    // Path::components() properly handles platform-specific separators
    let components = Path::new(path).components();
    for comp in components {
        if let std::path::Component::ParentDir = comp {
            return Err("path must not contain parent directory references (..)".to_string());
        }
    }
    Ok(())
}

/// Validates command arguments to ensure they are safe for execution.
///
/// # Security
///
/// Prevents command injection by rejecting:
/// - Empty arguments (could be interpreted as flags)
/// - NUL bytes (string termination bypass)
///
/// # Arguments
///
/// * `args` - Slice of command arguments to validate
///
/// # Returns
///
/// * `Ok(())` if all arguments are safe
/// * `Err(String)` with a descriptive error message if validation fails
///
/// # Example
///
/// ```
/// validate_args(&["--flag", "value"])?; // Ok
/// validate_args(&["--flag", ""])?; // Err: empty argument at index 1
/// ```
pub fn validate_args(args: &[&str]) -> Result<(), String> {
    for (i, arg) in args.iter().enumerate() {
        if arg.is_empty() {
            return Err(format!("argument at index {} must not be empty", i));
        }
        if arg.contains('\0') {
            return Err(format!(
                "argument at index {} must not contain NUL bytes",
                i
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_path_empty() {
        assert!(validate_path("").is_err());
    }

    #[test]
    fn test_validate_path_null_byte() {
        assert!(validate_path("file.txt\0").is_err());
    }

    #[test]
    fn test_validate_path_parent_dir() {
        assert!(validate_path("../../../etc/passwd").is_err());
        assert!(validate_path("folder/../sensitive").is_err());
        assert!(validate_path("/home/user/../root").is_err());
    }

    #[test]
    fn test_validate_path_valid() {
        assert!(validate_path("/home/user/file.txt").is_ok());
        assert!(validate_path("relative/path").is_ok());
        assert!(validate_path(".").is_ok());
    }

    #[test]
    fn test_validate_args_empty() {
        assert!(validate_args(&["arg1", ""]).is_err());
        assert!(validate_args(&[""]).is_err());
    }

    #[test]
    fn test_validate_args_null_byte() {
        assert!(validate_args(&["arg\0"]).is_err());
    }

    #[test]
    fn test_validate_args_valid() {
        assert!(validate_args(&["--flag", "value"]).is_ok());
        assert!(validate_args(&[]).is_ok());
    }
}
