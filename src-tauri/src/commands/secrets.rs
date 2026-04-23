//! OS-keyring-backed secret storage commands.
//!
//! Feeds the TypeScript `ISecretStorageService`. Keys are namespaced
//! automatically (the crate stores them under the `SideX` service id) so
//! collisions with other apps on the same keyring are impossible.

use std::sync::Arc;

use sidex_auth::SecretStorage;
use tauri::{AppHandle, Manager};

pub struct SecretsStore {
    inner: SecretStorage,
}

impl SecretsStore {
    fn new(storage: SecretStorage) -> Self {
        Self { inner: storage }
    }
}

pub fn initialize(app: &AppHandle) -> Result<(), String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app_data_dir: {e}"))?;
    let db_path = data_dir.join("UserData").join("secrets-index.db");
    let storage = SecretStorage::open(db_path).map_err(|e| e.to_string())?;
    app.manage(Arc::new(SecretsStore::new(storage)));
    Ok(())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn secret_get(
    store: tauri::State<'_, Arc<SecretsStore>>,
    key: String,
) -> Result<Option<String>, String> {
    store.inner.get(&key).map_err(|e| e.to_string())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn secret_set(
    store: tauri::State<'_, Arc<SecretsStore>>,
    key: String,
    value: String,
) -> Result<(), String> {
    store.inner.set(&key, &value).map_err(|e| e.to_string())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn secret_delete(
    store: tauri::State<'_, Arc<SecretsStore>>,
    key: String,
) -> Result<(), String> {
    store.inner.delete(&key).map_err(|e| e.to_string())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn secret_keys(store: tauri::State<'_, Arc<SecretsStore>>) -> Result<Vec<String>, String> {
    store.inner.keys().map_err(|e| e.to_string())
}
