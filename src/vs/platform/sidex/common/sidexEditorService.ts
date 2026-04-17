/*---------------------------------------------------------------------------------------------
 *  SideX Editor Bridge
 *  Intercepts high-level editor operations and forwards them to the Rust
 *  `sidex-editor` / `sidex-text` crates via Tauri IPC.
 *--------------------------------------------------------------------------------------------*/

import { invoke } from '../../../sidex-bridge.js';

export class SideXEditorBridge {
	private static instance: SideXEditorBridge;

	static getInstance(): SideXEditorBridge {
		if (!SideXEditorBridge.instance) {
			SideXEditorBridge.instance = new SideXEditorBridge();
		}
		return SideXEditorBridge.instance;
	}

	// --- File operations (sidex-text) ---

	async readFile(path: string): Promise<string> {
		return invoke<string>('read_file', { path });
	}

	async writeFile(path: string, content: string): Promise<void> {
		return invoke<void>('write_file', { path, content });
	}

	// --- Language detection (sidex-syntax) ---

	async detectLanguage(filename: string): Promise<string> {
		try {
			return await invoke<string>('syntax_detect_language', { filename });
		} catch {
			return 'plaintext';
		}
	}

	// --- Git status (sidex-git) ---

	async getGitStatus(repoRoot: string): Promise<any> {
		return invoke('git_status', { repoRoot });
	}

	// --- Search (sidex-workspace) ---

	async searchInFiles(dir: string, query: string): Promise<any> {
		return invoke('search_text', { dir, query });
	}

	// --- Settings (sidex-settings) ---

	async getSettings(section?: string): Promise<any> {
		return invoke('settings_get', { section: section ?? null });
	}

	// --- Theme (sidex-theme) ---

	async getThemeList(): Promise<any[]> {
		return (await invoke<any[]>('theme_list')) ?? [];
	}

	async getThemeData(id: string): Promise<any> {
		return invoke('theme_get', { id });
	}
}
