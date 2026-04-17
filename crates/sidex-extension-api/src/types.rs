//! Core VS Code API types: `Uri`, `Position`, `Range`, `Selection`,
//! `TextDocument`, `TextEditor`, `TextEditorEdit`, `DiagnosticCollection`,
//! `EventEmitter`, `Disposable`, `CancellationToken`, `FileSystemWatcher`,
//! `AuthenticationProvider`, `NotebookSerializer`, `CustomEditorProvider`.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, RwLock};

// ---------------------------------------------------------------------------
// Uri
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Uri {
    pub scheme: String,
    pub authority: String,
    pub path: String,
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub fragment: String,
}

impl Uri {
    pub fn file(path: &str) -> Self {
        Self { scheme: "file".into(), authority: String::new(), path: path.into(), query: String::new(), fragment: String::new() }
    }
    pub fn parse(raw: &str) -> Self {
        if let Some(rest) = raw.strip_prefix("file://") {
            return Self::file(rest);
        }
        if let Some((scheme, rest)) = raw.split_once("://") {
            let (authority, path) = rest.split_once('/').map(|(a, p)| (a.to_owned(), format!("/{p}"))).unwrap_or((String::new(), rest.to_owned()));
            return Self { scheme: scheme.into(), authority, path, query: String::new(), fragment: String::new() };
        }
        Self::file(raw)
    }
    pub fn to_string_repr(&self) -> String {
        if self.authority.is_empty() { format!("{}://{}", self.scheme, self.path) }
        else { format!("{}://{}{}", self.scheme, self.authority, self.path) }
    }
}

// ---------------------------------------------------------------------------
// Disposable & CancellationToken
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Disposable { disposed: Arc<AtomicBool> }
impl Disposable {
    pub fn new() -> Self { Self { disposed: Arc::new(AtomicBool::new(false)) } }
    pub fn dispose(&self) { self.disposed.store(true, Ordering::Release); }
    pub fn is_disposed(&self) -> bool { self.disposed.load(Ordering::Acquire) }
}
impl Default for Disposable { fn default() -> Self { Self::new() } }

#[derive(Debug, Clone)]
pub struct CancellationToken { cancelled: Arc<AtomicBool> }
impl CancellationToken {
    pub fn new() -> Self { Self { cancelled: Arc::new(AtomicBool::new(false)) } }
    pub fn cancel(&self) { self.cancelled.store(true, Ordering::Release); }
    pub fn is_cancelled(&self) -> bool { self.cancelled.load(Ordering::Acquire) }
}
impl Default for CancellationToken { fn default() -> Self { Self::new() } }

// ---------------------------------------------------------------------------
// EventEmitter
// ---------------------------------------------------------------------------

pub type EventCallback<T> = Arc<dyn Fn(&T) + Send + Sync>;

pub struct EventEmitter<T: 'static> {
    next_id: AtomicU32,
    listeners: RwLock<Vec<(u32, EventCallback<T>)>>,
}

impl<T> EventEmitter<T> {
    pub fn new() -> Self { Self { next_id: AtomicU32::new(1), listeners: RwLock::new(Vec::new()) } }
    pub fn on(&self, cb: EventCallback<T>) -> u32 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.listeners.write().expect("lock").push((id, cb));
        id
    }
    pub fn off(&self, id: u32) {
        self.listeners.write().expect("lock").retain(|(i, _)| *i != id);
    }
    pub fn fire(&self, event: &T) {
        for (_, cb) in self.listeners.read().expect("lock").iter() { cb(event); }
    }
}

impl<T> Default for EventEmitter<T> { fn default() -> Self { Self::new() } }

// ---------------------------------------------------------------------------
// Diagnostic types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticSeverity { Error = 0, Warning = 1, Information = 2, Hint = 3 }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub range: sidex_extensions::protocol::Range,
    pub message: String,
    pub severity: DiagnosticSeverity,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub code: Option<Value>,
    #[serde(default)]
    pub related_information: Vec<Value>,
    #[serde(default)]
    pub tags: Vec<u32>,
}

pub struct DiagnosticCollection {
    name: String,
    entries: RwLock<HashMap<String, Vec<Diagnostic>>>,
}

impl DiagnosticCollection {
    pub fn new(name: &str) -> Self { Self { name: name.to_owned(), entries: RwLock::new(HashMap::new()) } }
    pub fn name(&self) -> &str { &self.name }
    pub fn set(&self, uri: &str, diagnostics: Vec<Diagnostic>) {
        self.entries.write().expect("lock").insert(uri.to_owned(), diagnostics);
    }
    pub fn get(&self, uri: &str) -> Vec<Diagnostic> {
        self.entries.read().expect("lock").get(uri).cloned().unwrap_or_default()
    }
    pub fn delete(&self, uri: &str) { self.entries.write().expect("lock").remove(uri); }
    pub fn clear(&self) { self.entries.write().expect("lock").clear(); }
    pub fn has(&self, uri: &str) -> bool { self.entries.read().expect("lock").contains_key(uri) }
}

// ---------------------------------------------------------------------------
// Authentication provider
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticationSession {
    pub id: String,
    pub access_token: String,
    pub account: AuthenticationAccount,
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticationAccount {
    pub id: String,
    pub label: String,
}

pub type AuthenticationProviderHandler = Arc<dyn Fn(&str, Value) -> anyhow::Result<Value> + Send + Sync>;

pub struct AuthenticationProviderRegistry {
    providers: RwLock<HashMap<String, AuthenticationProviderHandler>>,
}

impl AuthenticationProviderRegistry {
    pub fn new() -> Self { Self { providers: RwLock::new(HashMap::new()) } }
    pub fn register(&self, id: &str, handler: AuthenticationProviderHandler) {
        self.providers.write().expect("lock").insert(id.to_owned(), handler);
    }
    pub fn unregister(&self, id: &str) { self.providers.write().expect("lock").remove(id); }
    pub fn get_session(&self, provider_id: &str, params: Value) -> anyhow::Result<Value> {
        let providers = self.providers.read().expect("lock");
        match providers.get(provider_id) {
            Some(h) => h("getSession", params),
            None => anyhow::bail!("authentication provider not found: {provider_id}"),
        }
    }
}

impl Default for AuthenticationProviderRegistry { fn default() -> Self { Self::new() } }

// ---------------------------------------------------------------------------
// Notebook serializer (stub)
// ---------------------------------------------------------------------------

pub type NotebookSerializerHandler = Arc<dyn Fn(&str, &[u8]) -> anyhow::Result<Value> + Send + Sync>;

pub struct NotebookSerializerRegistry {
    serializers: RwLock<HashMap<String, NotebookSerializerHandler>>,
}

impl NotebookSerializerRegistry {
    pub fn new() -> Self { Self { serializers: RwLock::new(HashMap::new()) } }
    pub fn register(&self, notebook_type: &str, handler: NotebookSerializerHandler) {
        self.serializers.write().expect("lock").insert(notebook_type.to_owned(), handler);
    }
    pub fn deserialize(&self, notebook_type: &str, data: &[u8]) -> anyhow::Result<Value> {
        let serializers = self.serializers.read().expect("lock");
        match serializers.get(notebook_type) {
            Some(h) => h("deserialize", data),
            None => anyhow::bail!("notebook serializer not found: {notebook_type}"),
        }
    }
}

impl Default for NotebookSerializerRegistry { fn default() -> Self { Self::new() } }

// ---------------------------------------------------------------------------
// Custom editor provider (stub)
// ---------------------------------------------------------------------------

pub type CustomEditorHandler = Arc<dyn Fn(&str, Value) -> anyhow::Result<Value> + Send + Sync>;

pub struct CustomEditorProviderRegistry {
    providers: RwLock<HashMap<String, CustomEditorHandler>>,
}

impl CustomEditorProviderRegistry {
    pub fn new() -> Self { Self { providers: RwLock::new(HashMap::new()) } }
    pub fn register(&self, view_type: &str, handler: CustomEditorHandler) {
        self.providers.write().expect("lock").insert(view_type.to_owned(), handler);
    }
    pub fn resolve(&self, view_type: &str, params: Value) -> anyhow::Result<Value> {
        let providers = self.providers.read().expect("lock");
        match providers.get(view_type) {
            Some(h) => h("resolve", params),
            None => anyhow::bail!("custom editor provider not found: {view_type}"),
        }
    }
}

impl Default for CustomEditorProviderRegistry { fn default() -> Self { Self::new() } }
