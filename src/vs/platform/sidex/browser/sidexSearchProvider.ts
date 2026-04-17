/*---------------------------------------------------------------------------------------------
 *  SideX — Search provider backed by our Rust ripgrep integration.
 *  Provides a lightweight API for components that need search results without
 *  going through the full VS Code ISearchService machinery.
 *--------------------------------------------------------------------------------------------*/

import { invoke } from '../../../sidex-bridge.js';

export interface FileSearchResult {
	path: string;
	matches: Array<{
		lineNumber: number;
		lineText: string;
		matchStart: number;
		matchEnd: number;
	}>;
}

export class SideXSearchProvider {
	async textSearch(
		directory: string,
		query: string,
		options: {
			caseSensitive?: boolean;
			wholeWord?: boolean;
			regex?: boolean;
			include?: string;
			exclude?: string;
			maxResults?: number;
		} = {}
	): Promise<FileSearchResult[]> {
		try {
			const results = await invoke('search_text', {
				dir: directory,
				query,
				caseSensitive: options.caseSensitive ?? false,
				wholeWord: options.wholeWord ?? false,
				regex: options.regex ?? false,
				include: options.include ?? '',
				exclude: options.exclude ?? '',
				maxResults: options.maxResults ?? 2000,
			});
			return (results as FileSearchResult[]) || [];
		} catch (e) {
			console.error('[SideX] Search failed:', e);
			return [];
		}
	}

	async fileSearch(directory: string, pattern: string): Promise<string[]> {
		try {
			const results = await invoke('search_files', { dir: directory, pattern });
			return (results as string[]) || [];
		} catch {
			return [];
		}
	}
}
