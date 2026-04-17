/*---------------------------------------------------------------------------------------------
 *  SideX — File system provider that routes through the sidex-bridge.
 *  Provides the same functionality as TauriFileSystemProvider but uses the
 *  centralised `invoke()` wrapper from sidex-bridge.ts, consistent with
 *  the other SideX services (editor, git, search, etc.).
 *
 *  This provider is additive — it does NOT replace TauriFileSystemProvider.
 *  It can be used by higher-level SideX code that prefers the bridge
 *  abstraction over direct @tauri-apps/api/core imports.
 *--------------------------------------------------------------------------------------------*/

import { invoke } from '../../../sidex-bridge.js';

export interface SideXFileStat {
	type: number;
	size: number;
	mtime: number;
	ctime: number;
	readonly: boolean;
}

export interface SideXDirEntry {
	name: string;
	path: string;
	type: 'file' | 'directory' | 'symlink';
}

export class SideXFileSystemProvider {
	async readFile(path: string): Promise<Uint8Array> {
		const bytes = await invoke<number[]>('read_file_bytes', { path });
		if (Array.isArray(bytes)) {
			return new Uint8Array(bytes);
		}
		return new Uint8Array(0);
	}

	async readFileText(path: string): Promise<string> {
		const content = await invoke<string>('read_file', { path });
		return typeof content === 'string' ? content : '';
	}

	async writeFile(path: string, content: Uint8Array): Promise<void> {
		await invoke('write_file_bytes', { path, content: Array.from(content) });
	}

	async writeFileText(path: string, content: string): Promise<void> {
		await invoke('write_file', { path, content });
	}

	async stat(path: string): Promise<SideXFileStat> {
		const raw = await invoke<{
			size: number;
			is_dir: boolean;
			is_file: boolean;
			is_symlink: boolean;
			modified: number;
			created: number;
			readonly: boolean;
		}>('stat', { path });

		let type = 0; // FileType.Unknown
		if (raw.is_dir) {
			type = 2; // FileType.Directory
		} else if (raw.is_symlink) {
			type = 64; // FileType.SymbolicLink
		} else if (raw.is_file) {
			type = 1; // FileType.File
		}

		return {
			type,
			size: raw.size,
			mtime: raw.modified * 1000,
			ctime: raw.created * 1000,
			readonly: raw.readonly,
		};
	}

	async readDir(path: string): Promise<SideXDirEntry[]> {
		const entries = await invoke<Array<{
			name: string;
			path: string;
			is_dir: boolean;
			is_file: boolean;
			is_symlink: boolean;
		}>>('read_dir', { path });

		if (!Array.isArray(entries)) {
			return [];
		}

		return entries.map(e => ({
			name: e.name,
			path: e.path,
			type: e.is_dir ? 'directory' as const
				: e.is_symlink ? 'symlink' as const
					: 'file' as const,
		}));
	}

	async mkdir(path: string): Promise<void> {
		await invoke('mkdir', { path, recursive: true });
	}

	async delete(path: string, recursive = false): Promise<void> {
		await invoke('remove', { path, recursive });
	}

	async rename(oldPath: string, newPath: string): Promise<void> {
		await invoke('rename', { oldPath, newPath });
	}

	async exists(path: string): Promise<boolean> {
		try {
			const result = await invoke<boolean>('exists', { path });
			return !!result;
		} catch {
			return false;
		}
	}
}
