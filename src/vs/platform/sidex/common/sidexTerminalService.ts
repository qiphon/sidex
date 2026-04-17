import { invoke } from '../../../sidex-bridge.js';

export interface TerminalProfile {
	name: string;
	shellPath: string;
	args: string[];
	icon: string;
}

export class SideXTerminalService {
	async getDefaultShell(): Promise<string> {
		try {
			return await invoke('get_default_shell') || '/bin/zsh';
		} catch {
			return '/bin/zsh';
		}
	}

	async getAvailableShells(): Promise<string[]> {
		try {
			return await invoke('get_available_shells') || [];
		} catch {
			return ['/bin/zsh', '/bin/bash'];
		}
	}

	async spawn(options: { shell?: string; cwd?: string; env?: Record<string, string> }): Promise<number> {
		return invoke('term_spawn', {
			shell: options.shell || null,
			cwd: options.cwd || null,
			args: [],
			env: options.env || null,
		});
	}

	async write(id: number, data: string): Promise<void> {
		return invoke('term_write', { id, data });
	}

	async read(id: number): Promise<string> {
		return await invoke('term_read', { id }) || '';
	}

	async resize(id: number, cols: number, rows: number): Promise<void> {
		return invoke('term_resize', { id, cols, rows });
	}

	async kill(id: number): Promise<void> {
		return invoke('term_kill', { id });
	}
}
