import { invoke } from '../../../sidex-bridge.js';

export class SideXExtensionService {
	async searchMarketplace(query: string, page: number = 0): Promise<any[]> {
		try {
			return await invoke('extension_search_marketplace', { query, page }) || [];
		} catch {
			return [];
		}
	}

	async getInstalled(): Promise<any[]> {
		try {
			return await invoke('list_installed_extensions') || [];
		} catch {
			return [];
		}
	}

	async install(id: string): Promise<void> {
		return invoke('install_extension', { extensionId: id });
	}

	async uninstall(id: string): Promise<void> {
		return invoke('uninstall_extension', { extensionId: id });
	}
}
