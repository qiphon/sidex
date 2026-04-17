import { invoke } from '../../../sidex-bridge.js';

export interface ThemeInfo {
	id: string;
	label: string;
	uiTheme: string;
}

export interface ThemeData {
	workbenchColors: Record<string, string>;
	tokenColors: Array<{
		scope: string[];
		settings: { foreground?: string; fontStyle?: string };
	}>;
}

export class SideXThemeService {
	async listThemes(): Promise<ThemeInfo[]> {
		try {
			return await invoke('theme_list') || [];
		} catch {
			return [];
		}
	}

	async getTheme(id: string): Promise<ThemeData | null> {
		try {
			return await invoke('theme_get', { id });
		} catch {
			return null;
		}
	}

	async getDefaultDark(): Promise<ThemeData | null> {
		try {
			return await invoke('theme_get_default_dark');
		} catch {
			return null;
		}
	}

	async getDefaultLight(): Promise<ThemeData | null> {
		try {
			return await invoke('theme_get_default_light');
		} catch {
			return null;
		}
	}
}
