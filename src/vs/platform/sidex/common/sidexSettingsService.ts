import { invoke } from '../../../sidex-bridge.js';

export class SideXSettingsService {
	async get(section?: string): Promise<Record<string, unknown>> {
		try {
			return await invoke('settings_get', { section: section || null }) || {};
		} catch {
			return {};
		}
	}

	async update(key: string, value: unknown, scope: string = 'user'): Promise<void> {
		return invoke('settings_update', { key, value: JSON.stringify(value), scope });
	}
}
