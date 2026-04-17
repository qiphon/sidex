/*---------------------------------------------------------------------------------------------
 *  SideX Syntax Service
 *  Routes language detection and syntax configuration through the Rust
 *  `sidex-syntax` crate via Tauri IPC.
 *--------------------------------------------------------------------------------------------*/

import { invoke } from '../../../sidex-bridge.js';

export interface LanguageInfo {
	id: string;
	name: string;
	extensions: string[];
	filenames: string[];
}

export interface LanguageConfig {
	lineComment: string | null;
	blockComment: [string, string] | null;
	brackets: [string, string][];
}

export class SideXSyntaxService {
	private languageCache: Map<string, LanguageInfo> = new Map();

	async getLanguages(): Promise<LanguageInfo[]> {
		try {
			const languages = await invoke<LanguageInfo[]>('syntax_get_languages');
			if (Array.isArray(languages)) {
				for (const lang of languages) {
					this.languageCache.set(lang.id, lang);
				}
				return languages;
			}
		} catch (e) {
			console.warn('[SideX] Failed to get languages from Rust:', e);
		}
		return [];
	}

	getCachedLanguage(id: string): LanguageInfo | undefined {
		return this.languageCache.get(id);
	}

	async detectLanguage(filename: string): Promise<string> {
		try {
			const result = await invoke<string>('syntax_detect_language', { filename });
			return typeof result === 'string' ? result : 'plaintext';
		} catch {
			return 'plaintext';
		}
	}

	async getLanguageConfig(languageId: string): Promise<LanguageConfig | null> {
		try {
			return await invoke<LanguageConfig>('syntax_get_language_config', { languageId });
		} catch {
			return null;
		}
	}
}
