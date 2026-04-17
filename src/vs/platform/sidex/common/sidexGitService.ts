/*---------------------------------------------------------------------------------------------
 *  SideX Git Service
 *  Routes git operations through the Rust `sidex-git` crate via Tauri IPC.
 *--------------------------------------------------------------------------------------------*/

import { invoke } from '../../../sidex-bridge.js';

export class SideXGitService {
	async status(repoRoot: string): Promise<any> {
		return invoke('git_status', { repoRoot });
	}

	async diff(repoRoot: string, file?: string): Promise<any> {
		return invoke('git_diff', { repoRoot, file: file ?? null });
	}

	async log(repoRoot: string, maxCount: number = 50): Promise<any> {
		return invoke('git_log', { repoRoot, maxCount });
	}

	async branches(repoRoot: string): Promise<any> {
		return invoke('git_branches', { repoRoot });
	}

	async commit(repoRoot: string, message: string): Promise<any> {
		return invoke('git_commit', { repoRoot, message });
	}

	async checkout(repoRoot: string, branch: string): Promise<any> {
		return invoke('git_checkout', { repoRoot, ref: branch });
	}

	async add(repoRoot: string, files: string[]): Promise<any> {
		return invoke('git_add', { repoRoot, paths: files });
	}

	async push(repoRoot: string): Promise<any> {
		return invoke('git_push', { repoRoot });
	}

	async pull(repoRoot: string): Promise<any> {
		return invoke('git_pull', { repoRoot });
	}
}
