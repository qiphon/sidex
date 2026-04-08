use serde::Serialize;
use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

// SECURITY: Use centralized validation to prevent path traversal (CWE-22)
use super::validation::validate_path;

#[derive(Debug, Serialize)]
pub struct DirEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub is_file: bool,
    pub is_symlink: bool,
    pub size: u64,
    pub modified: u64,
}

#[derive(Debug, Serialize)]
pub struct FileStat {
    pub size: u64,
    pub is_dir: bool,
    pub is_file: bool,
    pub is_symlink: bool,
    pub modified: u64,
    pub created: u64,
    pub readonly: bool,
}

#[tauri::command]
pub fn read_file(path: String) -> Result<String, String> {
    validate_path(&path)?;
    fs::read_to_string(&path).map_err(|e| format!("Failed to read file '{}': {}", path, e))
}

#[tauri::command]
pub fn read_file_bytes(path: String) -> Result<Vec<u8>, String> {
    validate_path(&path)?;
    fs::read(&path).map_err(|e| format!("Failed to read file '{}': {}", path, e))
}

#[tauri::command]
pub fn write_file(path: String, content: String) -> Result<(), String> {
    validate_path(&path)?;
    if let Some(parent) = Path::new(&path).parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create parent dirs for '{}': {}", path, e))?;
    }
    fs::write(&path, content).map_err(|e| format!("Failed to write file '{}': {}", path, e))
}

#[tauri::command]
pub fn write_file_bytes(path: String, content: Vec<u8>) -> Result<(), String> {
    validate_path(&path)?;
    if let Some(parent) = Path::new(&path).parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create parent dirs for '{}': {}", path, e))?;
    }
    fs::write(&path, content).map_err(|e| format!("Failed to write file '{}': {}", path, e))
}

#[tauri::command]
pub fn read_dir(path: String) -> Result<Vec<DirEntry>, String> {
    validate_path(&path)?;
    let entries =
        fs::read_dir(&path).map_err(|e| format!("Failed to read dir '{}': {}", path, e))?;

    let mut result = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let metadata = entry
            .metadata()
            .map_err(|e| format!("Failed to get metadata: {}", e))?;
        let file_type = entry
            .file_type()
            .map_err(|e| format!("Failed to get file type: {}", e))?;

        let modified = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        result.push(DirEntry {
            name: entry.file_name().to_string_lossy().to_string(),
            path: entry.path().to_string_lossy().to_string(),
            is_dir: file_type.is_dir(),
            is_file: file_type.is_file(),
            is_symlink: file_type.is_symlink(),
            size: metadata.len(),
            modified,
        });
    }

    result.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Ok(result)
}

#[tauri::command]
pub fn stat(path: String) -> Result<FileStat, String> {
    validate_path(&path)?;
    let metadata = fs::metadata(&path).map_err(|e| format!("Failed to stat '{}': {}", path, e))?;

    let modified = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let created = metadata
        .created()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);

    Ok(FileStat {
        size: metadata.len(),
        is_dir: metadata.is_dir(),
        is_file: metadata.file_type().is_file(),
        is_symlink: metadata.file_type().is_symlink(),
        modified,
        created,
        readonly: metadata.permissions().readonly(),
    })
}

#[tauri::command]
pub fn mkdir(path: String, recursive: bool) -> Result<(), String> {
    validate_path(&path)?;
    if recursive {
        fs::create_dir_all(&path)
    } else {
        fs::create_dir(&path)
    }
    .map_err(|e| format!("Failed to create dir '{}': {}", path, e))
}

#[tauri::command]
pub fn remove(path: String, recursive: bool) -> Result<(), String> {
    validate_path(&path)?;
    let meta = fs::metadata(&path).map_err(|e| format!("Failed to stat '{}': {}", path, e))?;

    if meta.is_dir() {
        if recursive {
            fs::remove_dir_all(&path)
        } else {
            fs::remove_dir(&path)
        }
    } else {
        fs::remove_file(&path)
    }
    .map_err(|e| format!("Failed to remove '{}': {}", path, e))
}

#[tauri::command]
pub fn rename(old_path: String, new_path: String) -> Result<(), String> {
    validate_path(&old_path)?;
    validate_path(&new_path)?;
    fs::rename(&old_path, &new_path)
        .map_err(|e| format!("Failed to rename '{}' -> '{}': {}", old_path, new_path, e))
}

#[tauri::command]
pub fn exists(path: String) -> bool {
    Path::new(&path).exists()
}
