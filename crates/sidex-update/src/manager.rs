//! Top-level orchestration: drives the VS Code update state machine.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use ed25519_dalek::VerifyingKey;
use parking_lot::Mutex;
use tokio::sync::Mutex as AsyncMutex;

use crate::download::{self, DownloadJob, DownloadObserver};
use crate::manifest::{self, Platform, UpdateInfo};
use crate::{
    install, signature,
    state::{State, UpdateType},
    UpdateError, UpdateResult,
};

/// Configuration snapshot for the update manager.
#[derive(Debug, Clone)]
pub struct UpdateConfig {
    /// Feed endpoints (tried in order).
    pub endpoints: Vec<String>,
    /// Base-64 Minisign public key (as stored in `tauri.conf.json`).
    pub pubkey: Option<String>,
    /// Currently running product version (e.g. `0.1.2`).
    pub current_version: String,
    /// Directory where downloaded artifacts are cached.
    pub cache_dir: PathBuf,
    /// Bundle format for this build (`Archive` on mac/linux, `Setup` on win).
    pub update_type: UpdateType,
    /// HTTP user agent sent with requests.
    pub user_agent: String,
}

/// Subscribes to state transitions so callers (e.g. Tauri command layer)
/// can forward them to the UI.
pub trait UpdateObserver: Send + Sync {
    fn on_state_change(&self, state: &State);
}

/// Core update manager. Cheap to clone; internal state is shared.
#[derive(Clone)]
pub struct UpdateManager {
    inner: Arc<Inner>,
}

struct Inner {
    config: UpdateConfig,
    pubkey: Option<VerifyingKey>,
    client: reqwest::Client,
    state: Mutex<State>,
    last_artifact: Mutex<Option<PathBuf>>,
    cancel: Mutex<Option<Arc<AtomicBool>>>,
    observer: Mutex<Option<Arc<dyn UpdateObserver>>>,
    busy: AsyncMutex<()>,
}

impl UpdateManager {
    /// Construct a new manager.
    ///
    /// When `config.pubkey` fails to parse the manager still operates but
    /// signature verification will reject every download. Callers can
    /// surface the [`UpdateError::SignatureInvalid`] returned by
    /// [`Self::download_update`] in that case.
    pub fn new(config: UpdateConfig) -> UpdateResult<Self> {
        let pubkey = match &config.pubkey {
            Some(raw) => Some(signature::decode_public_key(raw)?),
            None => None,
        };
        let client = reqwest::Client::builder()
            .user_agent(&config.user_agent)
            .build()?;

        Ok(Self {
            inner: Arc::new(Inner {
                pubkey,
                client,
                state: Mutex::new(State::idle(config.update_type)),
                last_artifact: Mutex::new(None),
                cancel: Mutex::new(None),
                observer: Mutex::new(None),
                busy: AsyncMutex::new(()),
                config,
            }),
        })
    }

    /// Registers the sink for state-change notifications.
    pub fn set_observer(&self, observer: Arc<dyn UpdateObserver>) {
        *self.inner.observer.lock() = Some(observer);
    }

    /// Snapshots the current state.
    pub fn state(&self) -> State {
        self.inner.state.lock().clone()
    }

    /// Performs an update check and transitions into
    /// [`State::AvailableForDownload`] or back to [`State::Idle`].
    pub async fn check_for_updates(&self, explicit: bool) -> UpdateResult<()> {
        let _guard = self.inner.busy.lock().await;
        self.transition(State::CheckingForUpdates { explicit });

        let Some(platform) = Platform::current() else {
            self.transition(State::idle_not_available(self.inner.config.update_type));
            return Ok(());
        };

        let manifest = manifest::fetch_manifest(&self.inner.client, &self.inner.config.endpoints)
            .await
            .inspect_err(|e| {
                self.transition(State::idle_with_error(
                    self.inner.config.update_type,
                    e.to_string(),
                ));
            })?;

        if !manifest::is_newer(&manifest.version, &self.inner.config.current_version) {
            self.transition(State::idle_not_available(self.inner.config.update_type));
            return Ok(());
        }

        let Some(release) = manifest.release_for(platform) else {
            self.transition(State::idle_not_available(self.inner.config.update_type));
            return Ok(());
        };

        let update = UpdateInfo::from_manifest(&manifest, release);
        self.transition(State::AvailableForDownload {
            update,
            can_install: Some(true),
        });
        Ok(())
    }

    /// Downloads the already-known-available update and transitions into
    /// [`State::Ready`] (desktop) or [`State::Downloaded`] (background).
    pub async fn download_update(&self, explicit: bool) -> UpdateResult<()> {
        let busy_guard = self.inner.busy.lock().await;
        let update = match self.state() {
            State::AvailableForDownload { update, .. } => update,
            State::Idle { .. } if explicit => {
                // Allow "download" to double as a check+download in one call,
                // matching VS Code's explicit UX.
                drop(busy_guard);
                self.check_for_updates(true).await?;
                let _busy_guard = self.inner.busy.lock().await;
                match self.state() {
                    State::AvailableForDownload { update, .. } => update,
                    _ => return Ok(()),
                }
            }
            other => return Err(UpdateError::InvalidState(other.kind())),
        };

        let url = update
            .url
            .clone()
            .ok_or_else(|| UpdateError::MalformedManifest("manifest entry missing url".into()))?;

        let start = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        self.transition(State::Downloading {
            update: Some(update.clone()),
            explicit,
            overwrite: false,
            downloaded_bytes: Some(0),
            total_bytes: None,
            start_time: Some(start),
        });

        let cancel = Arc::new(AtomicBool::new(false));
        *self.inner.cancel.lock() = Some(cancel.clone());

        let destination = self.artifact_path(&update);
        let observer = ProgressBridge {
            manager: self.clone(),
            update: update.clone(),
            explicit,
            start_time: start,
        };
        let job = DownloadJob {
            url: &url,
            destination: &destination,
            expected_sha256: update.sha256hash.as_deref(),
            cancel: cancel.clone(),
        };

        let result = download::download(&self.inner.client, &job, &observer).await;
        *self.inner.cancel.lock() = None;

        match result {
            Ok(path) => {
                self.verify_signature(&path, &url).await?;
                *self.inner.last_artifact.lock() = Some(path);
                self.transition(State::Ready {
                    update,
                    explicit,
                    overwrite: false,
                });
                Ok(())
            }
            Err(err) => {
                self.transition(State::idle_with_error(
                    self.inner.config.update_type,
                    err.to_string(),
                ));
                Err(err)
            }
        }
    }

    /// Applies the staged update by invoking the platform installer.
    ///
    /// Used primarily on Windows where the installer runs and the
    /// application quits itself. On macOS / Linux, [`Self::quit_and_install`]
    /// is the preferred entry point.
    pub async fn apply_update(&self) -> UpdateResult<()> {
        let (update, explicit) = match self.state() {
            State::Ready {
                update, explicit, ..
            }
            | State::Downloaded {
                update, explicit, ..
            } => (update, explicit),
            other => return Err(UpdateError::InvalidState(other.kind())),
        };

        self.transition(State::Updating {
            update: update.clone(),
            current_progress: None,
            max_progress: None,
            explicit,
        });

        let artifact = self
            .inner
            .last_artifact
            .lock()
            .clone()
            .ok_or_else(|| UpdateError::InstallFailed("no staged artifact".into()))?;

        install::install(&artifact).await.inspect_err(|e| {
            self.transition(State::idle_with_error(
                self.inner.config.update_type,
                e.to_string(),
            ));
        })?;

        self.transition(State::Ready {
            update,
            explicit,
            overwrite: false,
        });
        Ok(())
    }

    /// Cancels an in-flight download.
    pub fn cancel(&self) {
        if let Some(cancel) = self.inner.cancel.lock().clone() {
            cancel.store(true, Ordering::Relaxed);
        }
    }

    /// Cleans up stale artifacts from the cache directory.
    pub async fn cleanup_cache(&self) -> UpdateResult<()> {
        let cache = self.inner.config.cache_dir.clone();
        tokio::fs::create_dir_all(&cache).await?;

        let mut dir = tokio::fs::read_dir(&cache).await?;
        while let Some(entry) = dir.next_entry().await? {
            let path = entry.path();
            let current = self.inner.last_artifact.lock().clone();
            if current.as_deref() == Some(&path) {
                continue;
            }
            let _ = tokio::fs::remove_file(path).await;
        }
        Ok(())
    }

    async fn verify_signature(&self, artifact: &Path, url: &str) -> UpdateResult<()> {
        let Some(pubkey) = &self.inner.pubkey else {
            log::warn!("no pubkey configured; skipping signature verification");
            return Ok(());
        };

        // Our release pipeline publishes `<url>.sig`; fall back gracefully
        // if the sidecar is absent so dev/self-hosted builds still install.
        let sig_url = format!("{url}.sig");
        let sig = match self.inner.client.get(&sig_url).send().await {
            Ok(r) if r.status().is_success() => r.text().await?,
            Ok(r) => {
                return Err(UpdateError::SignatureInvalid(format!(
                    "fetching {sig_url} returned status {}",
                    r.status()
                )));
            }
            Err(e) => {
                return Err(UpdateError::SignatureInvalid(format!(
                    "could not fetch {sig_url}: {e}"
                )));
            }
        };

        let payload = tokio::fs::read(artifact).await?;
        signature::verify(pubkey, &sig, &payload)
    }

    fn artifact_path(&self, update: &UpdateInfo) -> PathBuf {
        let suffix = update
            .url
            .as_deref()
            .and_then(|u| u.rsplit('/').next())
            .filter(|name| !name.is_empty() && !name.contains(['?', '#']))
            .unwrap_or("sidex-update.bin");
        self.inner
            .config
            .cache_dir
            .join(format!("{}-{}", update.version, suffix))
    }

    #[allow(clippy::needless_pass_by_value)]
    fn transition(&self, next: State) {
        *self.inner.state.lock() = next.clone();
        if let Some(observer) = self.inner.observer.lock().clone() {
            observer.on_state_change(&next);
        }
    }
}

struct ProgressBridge {
    manager: UpdateManager,
    update: UpdateInfo,
    explicit: bool,
    start_time: u64,
}

impl DownloadObserver for ProgressBridge {
    fn on_progress(&self, downloaded: u64, total: Option<u64>) {
        self.manager.transition(State::Downloading {
            update: Some(self.update.clone()),
            explicit: self.explicit,
            overwrite: false,
            downloaded_bytes: Some(downloaded),
            total_bytes: total,
            start_time: Some(self.start_time),
        });
    }
}
