# Extension Host API Coverage Report

> Generated: 2026-05-07
> Extension host: `src-tauri/extension-host/host.cjs`
> Rust API handler: `crates/sidex-extension-api/src/api.rs`

## Summary

| Category | Implemented | Partial | Stubs | Total | Coverage |
|----------|------------|---------|-------|-------|----------|
| Core types (Position, Range, etc.) | 110+ | 0 | 0 | 110+ | ~100% |
| `vscode.commands` | 4 | 0 | 0 | 4 | 100% |
| `vscode.workspace` | 12 | 2 | 8 | 22 | ~55% |
| `vscode.window` | 22 | 1 | 11 | 34 | ~65% |
| `vscode.languages` | 22 | 0 | 10 | 32 | ~69% |
| `vscode.env` | 14 | 0 | 1 | 15 | ~93% |
| `vscode.extensions` | 2 | 0 | 1 | 3 | ~67% |
| `vscode.tasks` | 5 | 0 | 0 | 5 | 100% |
| `vscode.debug` | 12 | 0 | 2 | 14 | ~86% |
| `vscode.authentication` | 3 | 0 | 0 | 3 | 100% |
| `vscode.l10n` | 1 | 0 | 2 | 3 | ~33% |
| `vscode.notebooks` | 1 | 0 | 3 | 4 | ~25% |
| `vscode.scm` | 1 | 0 | 2 | 3 | ~33% |
| `vscode.comments` | 1 | 0 | 0 | 1 | 100% |
| `vscode.tests` | 1 | 0 | 3 | 4 | ~25% |
| `vscode.chat` | 1 | 0 | 2 | 3 | ~33% |
| `vscode.lm` | 1 | 0 | 4 | 5 | ~20% |

## Critical Gaps (P0/P1)

### File Events
- `onDidCreateFiles`, `onDidDeleteFiles`, `onDidRenameFiles` — all `noopEvent`
- `onWillSaveTextDocument` — `noopEvent`
- `onDidChangeWorkspaceFolders` — `noopEvent`

### Terminal Events
- `onDidOpenTerminal`, `onDidCloseTerminal`, `onDidChangeActiveTerminal` — all `noopEvent`

### Notebook APIs
- All notebook document events are `noopEvent`
- Notebook controller execution is a stub

### Language Model & Chat
- `vscode.lm.selectChatModels` returns `[]`
- `vscode.chat.createChatParticipant` is a stub

### Tab Groups
- `vscode.window.tabGroups` returns an empty stub

## Recommended Next Steps

1. **File events** — Connect the Rust file watcher to the extension host events
2. **Terminal events** — Wire terminal lifecycle to `onDidOpenTerminal`/`onDidCloseTerminal`
3. **Workspace folder events** — Trigger on workspace changes
4. **Notebook basics** — Implement notebook document events
