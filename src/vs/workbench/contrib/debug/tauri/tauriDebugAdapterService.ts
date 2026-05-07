/*---------------------------------------------------------------------------------------------
 *  Tauri Debug Adapter Service — bridges the Rust-side DebugAdapterRegistry with the
 *  TS AdapterManager. Handles:
 *  - Scanning installed extensions for debugger contributions
 *  - Marketplace search for debug adapters
 *  - Installing debug adapters from marketplace
 *  - Creating adapter descriptors for the Tauri wire protocol
 *--------------------------------------------------------------------------------------------*/

import { Disposable, IDisposable } from '../../../../base/common/lifecycle.js';
import { Emitter, Event } from '../../../../base/common/event.js';
import { IDebugAdapterDescriptorFactory, IDebugSession, IAdapterDescriptor } from '../common/debug.js';
import { TauriExecutableDebugAdapter } from './tauriDebugAdapter.js';

// Tauri IPC interface
declare global {
	const __TAURI__: {
		invoke(cmd: string, args?: Record<string, unknown>): Promise<unknown>;
		event: {
			listen(event: string, handler: (event: { payload: unknown }) => void): Promise<() => void>;
		};
	};
}

async function tauriInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
	if (typeof __TAURI__?.invoke === 'function') {
		return __TAURI__.invoke(cmd, args) as Promise<T>;
	}
	throw new Error(`Tauri invoke not available for command: ${cmd}`);
}

// ---------------------------------------------------------------------------
// Types (match Rust Tauri command responses)
// ---------------------------------------------------------------------------

export interface DebugAdapterInfo {
	typeName: string;
	command: string;
	args: string[];
	runtime?: string;
	commandLine: string;
	source?: string;
	extensionId?: string;
	label?: string;
	languages: string[];
}

export interface MarketplaceDebugAdapter {
	extensionId: string;
	extensionName: string;
	publisher: string;
	description: string;
	version: string;
	debugType: string;
	debugLabel: string;
	languages: string[];
	installCount: number;
	rating: number;
	iconUrl?: string;
	isInstalled: boolean;
}

export interface DapInstallAdapterResult {
	extensionId: string;
	extensionName: string;
	version: string;
	registeredAdapters: DebugAdapterInfo[];
}

// ---------------------------------------------------------------------------
// TauriDebugAdapterDescriptorFactory — creates descriptors for Tauri adapters
// ---------------------------------------------------------------------------

/**
 * Debug adapter descriptor factory for Tauri. When a debug session starts,
 * this factory resolves the adapter command line from the Rust-side registry
 * and creates a TauriExecutableDebugAdapter.
 */
export class TauriDebugAdapterDescriptorFactory extends Disposable implements IDebugAdapterDescriptorFactory {
	get type(): string {
		return '*'; // wildcard — handles all types via Rust registry
	}

	async createDebugAdapterDescriptor(session: IDebugSession): Promise<IAdapterDescriptor | undefined> {
		const registry = await this.getAdapterRegistry();
		const adapter = registry.find(a => a.typeName === session.configuration.type);
		if (!adapter) {
			return undefined;
		}

		const command = adapter.commandLine || adapter.command;
		if (!command) {
			return undefined;
		}

		return {
			type: 'executable',
			command,
			args: adapter.args || [],
			options: {
				cwd: undefined,
				env: undefined
			}
		};
	}

	private async getAdapterRegistry(): Promise<DebugAdapterInfo[]> {
		try {
			return await tauriInvoke<DebugAdapterInfo[]>('dap_get_adapter_registry');
		} catch {
			return [];
		}
	}
}

// ---------------------------------------------------------------------------
// TauriDebugAdapterService — marketplace discovery + install + scan
// ---------------------------------------------------------------------------

export class TauriDebugAdapterService extends Disposable {
	private readonly _onDidChange = this._register(new Emitter<void>());
	get onDidChange(): Event<void> {
		return this._onDidChange.event;
	}

	/**
	 * Scan installed extensions for debugger contributions and register them.
	 * Called at startup and after extension install/uninstall.
	 */
	async scanExtensionDebuggers(extensionsDir?: string): Promise<DebugAdapterInfo[]> {
		try {
			const dir = extensionsDir || await this.getDefaultExtensionsDir();
			const result = await tauriInvoke<DebugAdapterInfo[]>('dap_scan_extension_debuggers', {
				extensionsDir: dir
			});
			this._onDidChange.fire();
			return result;
		} catch (e) {
			console.warn('[TauriDebugAdapterService] Failed to scan extension debuggers:', e);
			return [];
		}
	}

	/**
	 * Search the marketplace for extensions that contribute debug adapters.
	 */
	async findMarketplaceAdapters(query?: string, extensionsDir?: string): Promise<MarketplaceDebugAdapter[]> {
		try {
			return await tauriInvoke<MarketplaceDebugAdapter[]>('dap_find_marketplace_adapters', {
				query: query || null,
				extensionsDir: extensionsDir || null
			});
		} catch (e) {
			console.warn('[TauriDebugAdapterService] Failed to search marketplace:', e);
			return [];
		}
	}

	/**
	 * Install a debug adapter extension from the marketplace and register it.
	 */
	async installAdapter(extensionId: string): Promise<DapInstallAdapterResult | undefined> {
		try {
			const result = await tauriInvoke<DapInstallAdapterResult>('dap_install_adapter', {
				extensionId
			});
			this._onDidChange.fire();
			return result;
		} catch (e) {
			console.warn('[TauriDebugAdapterService] Failed to install adapter:', e);
			return undefined;
		}
	}

	/**
	 * Unregister all debug adapters contributed by an extension (e.g. on uninstall).
	 */
	async unregisterExtensionAdapters(extensionId: string): Promise<string[]> {
		try {
			return await tauriInvoke<string[]>('dap_unregister_extension_adapters', {
				extensionId
			});
		} catch {
			return [];
		}
	}

	/**
	 * Register a debug adapter from an extension contribution.
	 */
	async registerAdapter(
		extensionId: string,
		debugType: string,
		label: string,
		program?: string,
		runtime?: string,
		args?: string[],
		extensionPath?: string,
		languages?: string[]
	): Promise<boolean> {
		try {
			return await tauriInvoke<boolean>('dap_register_extension_adapter', {
				extensionId,
				debugType,
				label,
				program: program || null,
				runtime: runtime || null,
				args: args || null,
				extensionPath: extensionPath || null,
				languages: languages || []
			});
		} catch {
			return false;
		}
	}

	/**
	 * Get all registered debug adapters.
	 */
	async getRegisteredAdapters(): Promise<DebugAdapterInfo[]> {
		try {
			return await tauriInvoke<DebugAdapterInfo[]>('dap_get_adapter_registry');
		} catch {
			return [];
		}
	}

	private async getDefaultExtensionsDir(): Promise<string> {
		try {
			const result = await tauriInvoke<string>('env_user_extensions_dir', {});
			return result;
		} catch {
			// Fallback — the Rust command has its own default
			return '';
		}
	}
}
