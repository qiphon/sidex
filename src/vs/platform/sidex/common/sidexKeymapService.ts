import { invoke } from '../../../sidex-bridge.js';

export interface KeybindingInfo {
	key: string;
	command: string;
	when: string | null;
	source: string;
}

export class SideXKeymapService {
	async getDefaults(): Promise<KeybindingInfo[]> {
		try {
			return await invoke('keymap_get_defaults') || [];
		} catch {
			return [];
		}
	}

	async getAll(): Promise<KeybindingInfo[]> {
		try {
			return await invoke('keymap_get_all') || [];
		} catch {
			return [];
		}
	}
}
