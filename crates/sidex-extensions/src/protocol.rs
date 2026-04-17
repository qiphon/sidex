//! Extension host JSON-RPC protocol types.
//!
//! Defines the bidirectional message protocol between the `SideX` main thread and
//! the Node.js extension host, modelled after VS Code's `extHost.protocol.ts`.
//!
//! The main thread sends [`MainToExtHost`] messages **to** extensions, and
//! receives [`ExtHostToMain`] messages **from** extensions. Both directions use
//! JSON-RPC 2.0 over stdin/stdout.

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ---------------------------------------------------------------------------
// Shared primitives
// ---------------------------------------------------------------------------

pub type Handle = u64;
pub type ExtensionId = String;
pub type DocumentUri = String;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Selection {
    pub anchor: Position,
    pub active: Position,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextEdit {
    pub range: Range,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionContext {
    pub trigger_kind: u32,
    #[serde(default)]
    pub trigger_character: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignatureHelpContext {
    pub trigger_kind: u32,
    #[serde(default)]
    pub trigger_character: Option<String>,
    pub is_retrigger: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceContext {
    pub include_declaration: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeActionContext {
    #[serde(default)]
    pub diagnostics: Vec<Value>,
    #[serde(default)]
    pub only: Option<Vec<String>>,
    #[serde(default)]
    pub trigger_kind: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormattingOptions {
    pub tab_size: u32,
    pub insert_spaces: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorInfo {
    pub uri: DocumentUri,
    pub selections: Vec<Selection>,
    pub visible_ranges: Vec<Range>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceFolder {
    pub uri: DocumentUri,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSelector {
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub scheme: Option<String>,
    #[serde(default)]
    pub pattern: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderMetadata {
    #[serde(default)]
    pub trigger_characters: Vec<String>,
    #[serde(default)]
    pub resolve_provider: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickPickOptions {
    #[serde(default)]
    pub place_holder: Option<String>,
    #[serde(default)]
    pub can_pick_many: bool,
    #[serde(default)]
    pub match_on_description: bool,
    #[serde(default)]
    pub match_on_detail: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickPickItem {
    pub label: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub detail: Option<String>,
    #[serde(default)]
    pub picked: bool,
    #[serde(default)]
    pub always_show: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InputBoxOptions {
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub password: bool,
    #[serde(default)]
    pub place_holder: Option<String>,
    #[serde(default)]
    pub ignore_focus_out: bool,
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShowTextDocumentOptions {
    #[serde(default)]
    pub view_column: Option<i32>,
    #[serde(default)]
    pub preserve_focus: bool,
    #[serde(default)]
    pub preview: bool,
    #[serde(default)]
    pub selection: Option<Range>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceEdit {
    pub edits: Vec<TextDocumentEdit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextDocumentEdit {
    pub uri: DocumentUri,
    pub edits: Vec<TextEdit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecorationData {
    pub range: Range,
    #[serde(default)]
    pub hover_message: Option<String>,
    #[serde(default)]
    pub render_options: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecorationRange {
    pub range: Range,
    #[serde(default)]
    pub hover_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalOptions {
    pub name: String,
    #[serde(default)]
    pub shell_path: Option<String>,
    #[serde(default)]
    pub shell_args: Vec<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub env: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebviewPanelOptions {
    #[serde(default)]
    pub enable_scripts: bool,
    #[serde(default)]
    pub retain_context_when_hidden: bool,
    #[serde(default)]
    pub local_resource_roots: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebviewOptions {
    #[serde(default)]
    pub enable_scripts: bool,
    #[serde(default)]
    pub enable_forms: bool,
    #[serde(default)]
    pub retain_context_when_hidden: bool,
    #[serde(default)]
    pub local_resource_roots: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenDialogOptions {
    #[serde(default)]
    pub default_uri: Option<String>,
    #[serde(default)]
    pub open_label: Option<String>,
    #[serde(default)]
    pub can_select_files: bool,
    #[serde(default)]
    pub can_select_folders: bool,
    #[serde(default)]
    pub can_select_many: bool,
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveDialogOptions {
    #[serde(default)]
    pub default_uri: Option<String>,
    #[serde(default)]
    pub save_label: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressOptions {
    #[serde(default)]
    pub location: u32,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub cancellable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeItem {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub tooltip: Option<String>,
    #[serde(default)]
    pub icon_path: Option<String>,
    #[serde(default)]
    pub collapsible_state: u32,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub context_value: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageSeverity { Info, Warning, Error }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RevealType { Default, InCenter, InCenterIfOutsideViewport, AtTop }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StatusBarAlignment { Left, Right }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewColumn { Active, Beside, One, Two, Three }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfigurationTarget { Global, Workspace, WorkspaceFolder }

/// Extension description sent to the host during initialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionDescription {
    pub identifier: ExtensionIdentifier,
    pub extension_location: String,
    #[serde(default)]
    pub activation_events: Vec<String>,
    #[serde(default)]
    pub main: Option<String>,
    #[serde(default)]
    pub browser: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionIdentifier {
    pub id: String,
    #[serde(default)]
    pub uuid: Option<String>,
}

/// Host environment sent during initialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostEnvironment {
    pub app_name: String,
    pub app_language: String,
    pub app_uri_scheme: String,
    #[serde(default)]
    pub global_storage_home: Option<String>,
    #[serde(default)]
    pub workspace_storage_home: Option<String>,
    #[serde(default)]
    pub log_level: u8,
}

/// Response error returned by the extension host.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

// ---------------------------------------------------------------------------
// Main thread → Extension host
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "params")]
pub enum MainToExtHost {
    // -- Lifecycle --
    #[serde(rename = "$initData")]
    InitData { extensions: Vec<ExtensionDescription>, environment: HostEnvironment },
    #[serde(rename = "$activateExtension")]
    ActivateExtension { extension_id: ExtensionId, activation_event: String },
    #[serde(rename = "$startExtensionHost")]
    StartExtensionHost { extensions: Vec<Value> },

    // -- Document sync --
    #[serde(rename = "$documentOpened")]
    DocumentOpened { uri: DocumentUri, language_id: String, version: i32, content: String },
    #[serde(rename = "$updateDocument")]
    UpdateDocument { uri: DocumentUri, changes: Vec<TextEdit>, version: u32 },
    #[serde(rename = "$documentClosed")]
    DocumentClosed { uri: DocumentUri },
    #[serde(rename = "$documentSaved")]
    DocumentSaved { uri: DocumentUri },

    // -- Editor state --
    #[serde(rename = "$setActiveEditor")]
    SetActiveEditor { uri: DocumentUri, selections: Vec<Selection>, visible_ranges: Vec<Range> },
    #[serde(rename = "$setVisibleEditors")]
    SetVisibleEditors { editors: Vec<EditorInfo> },
    #[serde(rename = "$activeEditorChanged")]
    ActiveEditorChanged { uri: Option<DocumentUri> },
    #[serde(rename = "$selectionChanged")]
    SelectionChanged { uri: DocumentUri, selections: Vec<Selection> },
    #[serde(rename = "$visibleRangesChanged")]
    VisibleRangesChanged { uri: DocumentUri, ranges: Vec<Range> },

    // -- Language features --
    #[serde(rename = "$provideCompletionItems")]
    ProvideCompletionItems { handle: Handle, uri: DocumentUri, position: Position, context: CompletionContext },
    #[serde(rename = "$provideHover")]
    ProvideHover { handle: Handle, uri: DocumentUri, position: Position },
    #[serde(rename = "$provideDefinition")]
    ProvideDefinition { handle: Handle, uri: DocumentUri, position: Position },
    #[serde(rename = "$provideReferences")]
    ProvideReferences { handle: Handle, uri: DocumentUri, position: Position, context: ReferenceContext },
    #[serde(rename = "$provideSignatureHelp")]
    ProvideSignatureHelp { handle: Handle, uri: DocumentUri, position: Position, context: SignatureHelpContext },
    #[serde(rename = "$provideDocumentSymbols")]
    ProvideDocumentSymbols { handle: Handle, uri: DocumentUri },
    #[serde(rename = "$provideCodeActions")]
    ProvideCodeActions { handle: Handle, uri: DocumentUri, range: Range, context: CodeActionContext },
    #[serde(rename = "$provideCodeLenses")]
    ProvideCodeLenses { handle: Handle, uri: DocumentUri },
    #[serde(rename = "$provideDocumentFormattingEdits")]
    ProvideDocumentFormattingEdits { handle: Handle, uri: DocumentUri, options: FormattingOptions },
    #[serde(rename = "$provideRenameEdits")]
    ProvideRenameEdits { handle: Handle, uri: DocumentUri, position: Position, new_name: String },
    #[serde(rename = "$resolveCompletionItem")]
    ResolveCompletionItem { handle: Handle, item: Value },
    #[serde(rename = "$resolveCodeLens")]
    ResolveCodeLens { handle: Handle, lens: Value },

    // -- Commands --
    #[serde(rename = "$executeCommand")]
    ExecuteCommand { id: String, args: Vec<Value> },

    // -- Configuration --
    #[serde(rename = "$setConfiguration")]
    SetConfiguration { data: Value },
    #[serde(rename = "$configurationChanged")]
    ConfigurationChanged { settings: Value },

    // -- Workspace --
    #[serde(rename = "$onDidChangeWorkspaceFolders")]
    OnDidChangeWorkspaceFolders { added: Vec<WorkspaceFolder>, removed: Vec<WorkspaceFolder> },

    // -- File system events --
    #[serde(rename = "$fileCreated")]
    FileCreated { uri: DocumentUri },
    #[serde(rename = "$fileChanged")]
    FileChanged { uri: DocumentUri },
    #[serde(rename = "$fileDeleted")]
    FileDeleted { uri: DocumentUri },

    // -- Webview --
    #[serde(rename = "$webviewMessage")]
    WebviewMessage { webview_id: String, message: Value },

    // -- Tree view --
    #[serde(rename = "$treeViewGetChildren")]
    TreeViewGetChildren { request_id: u64, view_id: String, element: Option<String> },
    #[serde(rename = "$treeViewSetExpanded")]
    TreeViewSetExpanded { view_id: String, element: String, expanded: bool },
    #[serde(rename = "$treeViewSetSelection")]
    TreeViewSetSelection { view_id: String, elements: Vec<String> },
    #[serde(rename = "$treeViewSetVisible")]
    TreeViewSetVisible { view_id: String, visible: bool },

    // -- Terminal --
    #[serde(rename = "$terminalCreated")]
    TerminalCreated { terminal_id: u32, name: String },
    #[serde(rename = "$terminalClosed")]
    TerminalClosed { terminal_id: u32 },
    #[serde(rename = "$terminalData")]
    TerminalData { terminal_id: u32, data: String },

    // -- Tasks --
    #[serde(rename = "$taskStarted")]
    TaskStarted { task_id: String },
    #[serde(rename = "$taskEnded")]
    TaskEnded { task_id: String, exit_code: Option<i32> },

    // -- Debug --
    #[serde(rename = "$debugSessionStarted")]
    DebugSessionStarted { session_id: String, debug_type: String },
    #[serde(rename = "$debugSessionTerminated")]
    DebugSessionTerminated { session_id: String },
}

// ---------------------------------------------------------------------------
// Extension host → Main thread
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "params")]
pub enum ExtHostToMain {
    // -- Provider registration --
    #[serde(rename = "$registerProvider")]
    RegisterProvider { handle: Handle, selector: Vec<DocumentSelector>, metadata: ProviderMetadata },

    // -- Responses --
    #[serde(rename = "$response")]
    Response { request_id: u64, result: Value },
    #[serde(rename = "$responseError")]
    ResponseError { request_id: u64, error: ResponseError },

    // -- UI: messages --
    #[serde(rename = "$showInformationMessage")]
    ShowInformationMessage { message: String, #[serde(default)] items: Vec<String> },
    #[serde(rename = "$showWarningMessage")]
    ShowWarningMessage { message: String, #[serde(default)] items: Vec<String> },
    #[serde(rename = "$showErrorMessage")]
    ShowErrorMessage { message: String, #[serde(default)] items: Vec<String> },
    #[serde(rename = "$showMessage")]
    ShowMessage { severity: MessageSeverity, message: String, #[serde(default)] actions: Vec<String> },

    // -- UI: quick pick / input box --
    #[serde(rename = "$showQuickPick")]
    ShowQuickPick { items: Vec<Value>, options: QuickPickOptions },
    #[serde(rename = "$showInputBox")]
    ShowInputBox { options: InputBoxOptions },
    #[serde(rename = "$showOpenDialog")]
    ShowOpenDialog { options: OpenDialogOptions },
    #[serde(rename = "$showSaveDialog")]
    ShowSaveDialog { options: SaveDialogOptions },

    // -- Editor operations --
    #[serde(rename = "$applyWorkspaceEdit")]
    ApplyWorkspaceEdit { edit: WorkspaceEdit },
    #[serde(rename = "$insertSnippet")]
    InsertSnippet { uri: DocumentUri, snippet: String, ranges: Vec<Range> },
    #[serde(rename = "$setDecorations")]
    SetDecorations { handle: Handle, uri: DocumentUri, decorations: Vec<DecorationData> },
    #[serde(rename = "$revealRange")]
    RevealRange { uri: DocumentUri, range: Range, reveal_type: RevealType },
    #[serde(rename = "$showTextDocument")]
    ShowTextDocument { uri: DocumentUri, options: ShowTextDocumentOptions },

    // -- Document operations --
    #[serde(rename = "$openDocument")]
    OpenDocument { uri: DocumentUri },
    #[serde(rename = "$saveDocument")]
    SaveDocument { uri: DocumentUri },

    // -- Output channels --
    #[serde(rename = "$createOutputChannel")]
    CreateOutputChannel { name: String },
    #[serde(rename = "$appendToOutputChannel")]
    AppendToOutputChannel { id: Handle, text: String },
    #[serde(rename = "$clearOutputChannel")]
    ClearOutputChannel { channel: String },

    // -- Terminal --
    #[serde(rename = "$createTerminal")]
    CreateTerminal { options: TerminalOptions },
    #[serde(rename = "$sendTerminalInput")]
    SendTerminalInput { id: Handle, text: String },
    #[serde(rename = "$sendTerminalData")]
    SendTerminalData { terminal_id: u32, data: String },

    // -- Status bar --
    #[serde(rename = "$setStatusBarMessage")]
    SetStatusBarMessage { text: String, #[serde(default)] timeout: Option<u64> },
    #[serde(rename = "$setStatusBarItem")]
    SetStatusBarItem {
        id: String, text: String,
        #[serde(default)] tooltip: Option<String>,
        #[serde(default)] command: Option<String>,
        alignment: StatusBarAlignment, priority: i32,
    },
    #[serde(rename = "$disposeStatusBarItem")]
    DisposeStatusBarItem { id: String },

    // -- Commands --
    #[serde(rename = "$registerCommand")]
    RegisterCommand { id: String },
    #[serde(rename = "$executeCommand")]
    ExecuteCommand { command: String, #[serde(default)] args: Vec<Value> },

    // -- Configuration --
    #[serde(rename = "$getConfiguration")]
    GetConfiguration { section: String },
    #[serde(rename = "$updateConfiguration")]
    UpdateConfiguration { section: String, value: Value, target: ConfigurationTarget },

    // -- File system --
    #[serde(rename = "$readFile")]
    ReadFile { uri: DocumentUri },
    #[serde(rename = "$writeFile")]
    WriteFile { uri: DocumentUri, content: Vec<u8> },
    #[serde(rename = "$stat")]
    Stat { uri: DocumentUri },
    #[serde(rename = "$readDir")]
    ReadDir { uri: DocumentUri },
    #[serde(rename = "$createDir")]
    CreateDir { uri: DocumentUri },
    #[serde(rename = "$deleteFile")]
    DeleteFile { uri: DocumentUri, recursive: bool },
    #[serde(rename = "$rename")]
    Rename { old_uri: DocumentUri, new_uri: DocumentUri },

    // -- Webview --
    #[serde(rename = "$createWebviewPanel")]
    CreateWebviewPanel { view_type: String, title: String, column: i32, options: WebviewPanelOptions },
    #[serde(rename = "$postMessageToWebview")]
    PostMessageToWebview { handle: Handle, message: Value },
    #[serde(rename = "$setWebviewHtml")]
    SetWebviewHtml { handle: Handle, html: String },

    // -- Tree views --
    #[serde(rename = "$registerTreeDataProvider")]
    RegisterTreeDataProvider { view_id: String, handle: Handle },
    #[serde(rename = "$refreshTreeView")]
    RefreshTreeView { view_id: String },
    #[serde(rename = "$treeViewGetChildrenResponse")]
    TreeViewGetChildrenResponse { request_id: u64, items: Vec<TreeItem> },

    // -- Progress --
    #[serde(rename = "$createProgress")]
    CreateProgress { id: String, options: ProgressOptions },
    #[serde(rename = "$updateProgress")]
    UpdateProgress { id: String, #[serde(default)] message: Option<String>, #[serde(default)] increment: Option<f32> },
    #[serde(rename = "$endProgress")]
    EndProgress { id: String },

    // -- Telemetry --
    #[serde(rename = "$logTelemetry")]
    LogTelemetry { event_name: String, data: Value },
}

// ---------------------------------------------------------------------------
// Wire-level JSON-RPC envelope
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcMessage {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl RpcMessage {
    pub fn request(id: u64, method: &str, params: Value) -> Self {
        Self { jsonrpc: "2.0".into(), id: Some(id), method: Some(method.into()), params: Some(params), result: None, error: None }
    }

    pub fn notification(method: &str, params: Value) -> Self {
        Self { jsonrpc: "2.0".into(), id: None, method: Some(method.into()), params: Some(params), result: None, error: None }
    }

    pub fn success(id: u64, result: Value) -> Self {
        Self { jsonrpc: "2.0".into(), id: Some(id), method: None, params: None, result: Some(result), error: None }
    }

    pub fn error_response(id: u64, code: i64, message: impl Into<String>) -> Self {
        Self { jsonrpc: "2.0".into(), id: Some(id), method: None, params: None, result: None,
            error: Some(RpcError { code, message: message.into(), data: None }) }
    }

    pub fn is_request(&self) -> bool { self.id.is_some() && self.method.is_some() }
    pub fn is_notification(&self) -> bool { self.id.is_none() && self.method.is_some() }
    pub fn is_response(&self) -> bool { self.id.is_some() && self.method.is_none() }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_main_to_ext_activate() {
        let msg = MainToExtHost::ActivateExtension {
            extension_id: "rust-lang.rust-analyzer".into(),
            activation_event: "onLanguage:rust".into(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("$activateExtension"));
        let back: MainToExtHost = serde_json::from_str(&json).unwrap();
        match back {
            MainToExtHost::ActivateExtension { extension_id, activation_event } => {
                assert_eq!(extension_id, "rust-lang.rust-analyzer");
                assert_eq!(activation_event, "onLanguage:rust");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn roundtrip_main_to_ext_provide_hover() {
        let msg = MainToExtHost::ProvideHover {
            handle: 42,
            uri: "file:///test.rs".into(),
            position: Position { line: 10, character: 5 },
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: MainToExtHost = serde_json::from_str(&json).unwrap();
        match back {
            MainToExtHost::ProvideHover { handle, uri, position } => {
                assert_eq!(handle, 42);
                assert_eq!(uri, "file:///test.rs");
                assert_eq!(position.line, 10);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn roundtrip_ext_to_main_show_message() {
        let msg = ExtHostToMain::ShowInformationMessage {
            message: "Hello!".into(),
            items: vec!["OK".into(), "Cancel".into()],
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("$showInformationMessage"));
        let back: ExtHostToMain = serde_json::from_str(&json).unwrap();
        match back {
            ExtHostToMain::ShowInformationMessage { message, items } => {
                assert_eq!(message, "Hello!");
                assert_eq!(items.len(), 2);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn roundtrip_ext_to_main_register_provider() {
        let msg = ExtHostToMain::RegisterProvider {
            handle: 7,
            selector: vec![DocumentSelector { language: Some("rust".into()), scheme: Some("file".into()), pattern: None }],
            metadata: ProviderMetadata { trigger_characters: vec![".".into(), ":".into()], resolve_provider: true },
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: ExtHostToMain = serde_json::from_str(&json).unwrap();
        match back {
            ExtHostToMain::RegisterProvider { handle, selector, metadata } => {
                assert_eq!(handle, 7);
                assert_eq!(selector.len(), 1);
                assert_eq!(selector[0].language.as_deref(), Some("rust"));
                assert!(metadata.resolve_provider);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn roundtrip_ext_to_main_workspace_edit() {
        let msg = ExtHostToMain::ApplyWorkspaceEdit {
            edit: WorkspaceEdit {
                edits: vec![TextDocumentEdit {
                    uri: "file:///test.rs".into(),
                    edits: vec![TextEdit {
                        range: Range {
                            start: Position { line: 0, character: 0 },
                            end: Position { line: 0, character: 5 },
                        },
                        text: "hello".into(),
                    }],
                }],
            },
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: ExtHostToMain = serde_json::from_str(&json).unwrap();
        match back {
            ExtHostToMain::ApplyWorkspaceEdit { edit } => {
                assert_eq!(edit.edits.len(), 1);
                assert_eq!(edit.edits[0].edits[0].text, "hello");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn rpc_message_request() {
        let msg = RpcMessage::request(1, "$provideHover", serde_json::json!({"uri": "file:///a"}));
        assert!(msg.is_request());
        assert!(!msg.is_notification());
        assert!(!msg.is_response());
    }

    #[test]
    fn rpc_message_notification() {
        let msg = RpcMessage::notification("$log", serde_json::json!({"text": "hi"}));
        assert!(msg.is_notification());
        assert!(!msg.is_request());
    }

    #[test]
    fn rpc_message_success_response() {
        let msg = RpcMessage::success(42, serde_json::json!({"items": []}));
        assert!(msg.is_response());
        assert_eq!(msg.id, Some(42));
        assert!(msg.result.is_some());
        assert!(msg.error.is_none());
    }

    #[test]
    fn rpc_message_error_response() {
        let msg = RpcMessage::error_response(99, -32600, "Invalid Request");
        assert!(msg.is_response());
        let err = msg.error.as_ref().unwrap();
        assert_eq!(err.code, -32600);
        assert_eq!(err.message, "Invalid Request");
    }

    #[test]
    fn roundtrip_main_to_ext_completion() {
        let msg = MainToExtHost::ProvideCompletionItems {
            handle: 1, uri: "file:///main.rs".into(),
            position: Position { line: 5, character: 12 },
            context: CompletionContext { trigger_kind: 1, trigger_character: Some(".".into()) },
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: MainToExtHost = serde_json::from_str(&json).unwrap();
        match back {
            MainToExtHost::ProvideCompletionItems { handle, context, .. } => {
                assert_eq!(handle, 1);
                assert_eq!(context.trigger_character.as_deref(), Some("."));
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn roundtrip_ext_to_main_create_webview() {
        let msg = ExtHostToMain::CreateWebviewPanel {
            view_type: "markdown.preview".into(), title: "Preview".into(), column: 2,
            options: WebviewPanelOptions { enable_scripts: true, retain_context_when_hidden: false, local_resource_roots: vec!["/workspace".into()] },
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: ExtHostToMain = serde_json::from_str(&json).unwrap();
        match back {
            ExtHostToMain::CreateWebviewPanel { view_type, title, options, .. } => {
                assert_eq!(view_type, "markdown.preview");
                assert_eq!(title, "Preview");
                assert!(options.enable_scripts);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn roundtrip_document_opened() {
        let msg = MainToExtHost::DocumentOpened {
            uri: "file:///test.rs".into(), language_id: "rust".into(), version: 1, content: "fn main() {}".into(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: MainToExtHost = serde_json::from_str(&json).unwrap();
        match back {
            MainToExtHost::DocumentOpened { uri, language_id, version, content } => {
                assert_eq!(uri, "file:///test.rs");
                assert_eq!(language_id, "rust");
                assert_eq!(version, 1);
                assert_eq!(content, "fn main() {}");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn roundtrip_file_operations() {
        let msg = ExtHostToMain::ReadFile { uri: "file:///test.rs".into() };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("$readFile"));

        let msg = ExtHostToMain::WriteFile { uri: "file:///test.rs".into(), content: vec![104, 105] };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("$writeFile"));
    }

    #[test]
    fn roundtrip_progress() {
        let msg = ExtHostToMain::CreateProgress {
            id: "p1".into(),
            options: ProgressOptions { location: 15, title: Some("Building...".into()), cancellable: true },
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: ExtHostToMain = serde_json::from_str(&json).unwrap();
        match back {
            ExtHostToMain::CreateProgress { id, options } => {
                assert_eq!(id, "p1");
                assert_eq!(options.title.as_deref(), Some("Building..."));
                assert!(options.cancellable);
            }
            _ => panic!("wrong variant"),
        }
    }
}
