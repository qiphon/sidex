/*---------------------------------------------------------------------------------------------
 *  Tauri Debug Contribution — wires the Rust-side DebugAdapterRegistry and marketplace
 *  discovery into the TS debug system. Runs as a workbench contribution at startup.
 *--------------------------------------------------------------------------------------------*/

import { Disposable } from '../../../../base/common/lifecycle.js';
import { IInstantiationService } from '../../../../platform/instantiation/common/instantiation.js';
import { IQuickInputService, IQuickPickItem } from '../../../../platform/quickinput/common/quickInput.js';
import { IEditorService } from '../../../services/editor/common/editorService.js';
import { IExtensionService } from '../../../services/extensions/common/extensions.js';
import { ILifecycleService, LifecyclePhase } from '../../../services/lifecycle/common/lifecycle.js';
import { IDialogService } from '../../../../platform/dialogs/common/dialogs.js';
import { ICommandService } from '../../../../platform/commands/common/commands.js';
import { ID } from '../common/debug.js';
import { IDebugService, IAdapterManager } from '../common/debug.js';
import { AdapterManager } from './debugAdapterManager.js';
import { TauriDebugAdapterService, TauriDebugAdapterDescriptorFactory, MarketplaceDebugAdapter } from '../tauri/tauriDebugAdapterService.js';
import { IWorkspaceContextService } from '../../../../platform/workspace/common/workspace.js';
import * as nls from '../../../../nls.js';
import Severity from '../../../../base/common/severity.js';
import { KeybindingsRegistry } from '../../../../platform/keybinding/common/keybindingsRegistry.js';
import { KeybindingWeight } from '../../../../platform/keybinding/common/keybindingsRegistry.js';
import { IExtensionsWorkbenchService } from '../../extensions/common/extensions.js';

/**
 * Workbench contribution that bridges the Rust debug adapter registry with the
 * TS AdapterManager. This:
 * 1. Scans installed extensions for debugger contributions at startup
 * 2. Registers a Tauri adapter descriptor factory for all debug types
 * 3. Overrides `debug.installAdditionalDebuggers` to use marketplace discovery
 */
export class TauriDebugContribution extends Disposable {
	private readonly tauriService: TauriDebugAdapterService;
	private readonly descriptorFactory: TauriDebugAdapterDescriptorFactory;

	constructor(
		@IInstantiationService private readonly instantiationService: IInstantiationService,
		@IDebugService private readonly debugService: IDebugService,
		@IQuickInputService private readonly quickInputService: IQuickInputService,
		@ICommandService private readonly commandService: ICommandService,
		@IDialogService private readonly dialogService: IDialogService,
		@IExtensionService private readonly extensionService: IExtensionService,
		@ILifecycleService private readonly lifecycleService: ILifecycleService,
		@IWorkspaceContextService private readonly workspaceService: IWorkspaceContextService
	) {
		super();

		this.tauriService = this.instantiationService.createInstance(TauriDebugAdapterService);
		this.descriptorFactory = new TauriDebugAdapterDescriptorFactory();
		this._register(this.tauriService);

		// Override the installAdditionalDebuggers command to use Tauri marketplace discovery
		this.registerInstallCommandOverride();
	}

	/**
	 * Called by the lifecycle system after the workbench is restored.
	 */
	async initialize(): Promise<void> {
		// Scan extension debuggers after extensions are ready
		await this.lifecycleService.when(LifecyclePhase.Restored);
		await this.scanExtensionDebuggers();

		// Register the Tauri descriptor factory with the AdapterManager
		this.registerAdapterFactory();

		// Listen for extension install/uninstall events
		this._register(
			this.extensionService.onDidChangeExtensions(() => {
				this.scanExtensionDebuggers();
			})
		);
	}

	/**
	 * Get the Tauri debug adapter service for external use.
	 */
	getService(): TauriDebugAdapterService {
		return this.tauriService;
	}

	private registerInstallCommandOverride(): void {
		// Re-register the command with Tauri-specific behavior
		KeybindingsRegistry.registerCommandAndKeybindingRule({
			id: 'debug.installAdditionalDebuggers',
			weight: KeybindingWeight.WorkbenchContrib,
			when: undefined,
			primary: undefined,
			handler: async (_accessor, query: string) => {
				await this.installAdditionalDebuggers(query);
			}
		});
	}

	private async scanExtensionDebuggers(): Promise<void> {
		const adapters = await this.tauriService.scanExtensionDebuggers();
		if (adapters.length > 0) {
			// New debuggers discovered — the AdapterManager schema update
			// happens automatically when debuggers are registered via the
			// extension point handler.
		}
	}

	private registerAdapterFactory(): void {
		const adapterManager = this.debugService.getAdapterManager();
		if (adapterManager instanceof AdapterManager) {
			// Register a descriptor factory that uses the Rust registry as a fallback
			// for any debug type that doesn't have a dedicated extension factory.
			this._register(
				adapterManager.registerDebugAdapterDescriptorFactory(this.descriptorFactory)
			);
		}
	}

	/**
	 * Show a quick pick UI for installing additional debug adapters from the marketplace.
	 */
	async installAdditionalDebuggers(languageLabel?: string): Promise<void> {
		const query = languageLabel || 'debug';

		// Show loading indicator
		const items = await this.tauriService.findMarketplaceAdapters(query);

		if (items.length === 0) {
			// Fallback to extensions search if marketplace discovery returns nothing
			const extensionsWorkbenchService = this.instantiationService.get(IExtensionsWorkbenchService);
			let searchFor = `@category:debuggers`;
			if (typeof languageLabel === 'string') {
				searchFor += ` ${languageLabel}`;
			}
			return extensionsWorkbenchService.openSearch(searchFor);
		}

		// Group by installed status
		const installedItems = items.filter(i => i.isInstalled);
		const availableItems = items.filter(i => !i.isInstalled);

		const picks: (IQuickPickItem & { adapter?: MarketplaceDebugAdapter })[] = [];

		if (installedItems.length > 0) {
			picks.push({
				type: 'separator',
				label: nls.localize('installed', 'Installed')
			});
			for (const item of installedItems) {
				picks.push({
					label: item.extensionName,
					description: `${item.publisher} • ${item.debugType}`,
					detail: item.description,
					iconPath: item.iconUrl ? { light: item.iconUrl, dark: item.iconUrl } : undefined,
					adapter: item
				});
			}
		}

		picks.push({
			type: 'separator',
			label: nls.localize('available', 'Available')
		});
		for (const item of availableItems) {
			picks.push({
				label: item.extensionName,
				description: `${item.publisher} • ${item.debugType}`,
				detail: item.description,
				iconPath: item.iconUrl ? { light: item.iconUrl, dark: item.iconUrl } : undefined,
				adapter: item
			});
		}

		const selected = await this.quickInputService.pick(picks, {
			placeHolder: nls.localize(
				'selectDebugAdapter',
				"Select a debug adapter to install for '{0}'",
				query
			)
		});

		if (selected?.adapter) {
			const adapter = selected.adapter;
			if (adapter.isInstalled) {
				await this.dialogService.info(
					nls.localize('alreadyInstalled', 'Already Installed'),
					nls.localize(
						'alreadyInstalledDetail',
						"'{0}' is already installed.",
						adapter.extensionName
					)
				);
				return;
			}

			const { confirmed } = await this.dialogService.confirm({
				type: Severity.Info,
				message: nls.localize('installConfirm', 'Install Debug Adapter'),
				detail: nls.localize(
					'installConfirmDetail',
					"Install '{0}' by {1}?\n\nType: {2}\nLanguages: {3}",
					adapter.extensionName,
					adapter.publisher,
					adapter.debugType,
					adapter.languages.join(', ') || 'N/A'
				),
				primaryButton: nls.localize({ key: 'install', comment: ['&& denotes a mnemonic'] }, '&&Install')
			});

			if (confirmed) {
				const result = await this.tauriService.installAdapter(adapter.extensionId);
				if (result) {
					await this.dialogService.info(
						nls.localize('installSuccess', 'Installed'),
						nls.localize(
							'installSuccessDetail',
							"'{0}' was installed successfully.{1}",
							adapter.extensionName,
							result.registeredAdapters.length > 0
								? ` Registered debug adapters: ${result.registeredAdapters.map(a => a.typeName).join(', ')}`
								: ''
						)
					);
				} else {
					await this.dialogService.error(
						nls.localize('installFailed', 'Installation Failed'),
						nls.localize(
							'installFailedDetail',
							"Failed to install '{0}'.",
							adapter.extensionName
						)
					);
				}
			}
		}
	}
}
