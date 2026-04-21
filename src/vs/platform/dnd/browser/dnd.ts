/*---------------------------------------------------------------------------------------------
 *  Copyright (c) Microsoft Corporation. All rights reserved.
 *  Licensed under the MIT License. See License.txt in the project root for license information.
 *--------------------------------------------------------------------------------------------*/

import { DataTransfers } from '../../../base/browser/dnd.js';
import { mainWindow } from '../../../base/browser/window.js';
import { DragMouseEvent } from '../../../base/browser/mouseEvent.js';
import { coalesce } from '../../../base/common/arrays.js';
import { DeferredPromise } from '../../../base/common/async.js';
import { VSBuffer } from '../../../base/common/buffer.js';
import { IDisposable, toDisposable } from '../../../base/common/lifecycle.js';
import { ResourceMap } from '../../../base/common/map.js';
import { parse } from '../../../base/common/marshalling.js';
import { Schemas } from '../../../base/common/network.js';
import { isNative, isWeb } from '../../../base/common/platform.js';
import { URI, UriComponents } from '../../../base/common/uri.js';
import { localize } from '../../../nls.js';
import { IDialogService } from '../../dialogs/common/dialogs.js';
import { IBaseTextResourceEditorInput, ITextEditorSelection } from '../../editor/common/editor.js';
import { HTMLFileSystemProvider } from '../../files/browser/htmlFileSystemProvider.js';
import { WebFileSystemAccess } from '../../files/browser/webFileSystemAccess.js';
import { ByteSize, IFileService } from '../../files/common/files.js';
import { IInstantiationService, ServicesAccessor } from '../../instantiation/common/instantiation.js';
import { extractSelection } from '../../opener/common/opener.js';
import { Registry } from '../../registry/common/platform.js';
import { IMarker } from '../../markers/common/markers.js';

// Tauri: Import invoke for getting file paths
let tauriInvoke: ((cmd: string, args?: any) => Promise<any>) | undefined = undefined;
try {
	// Dynamically import Tauri API when available
	const tauriModule = (globalThis as any)['__TAURI__']?.core;
	if (tauriModule && typeof tauriModule.invoke === 'function') {
		tauriInvoke = tauriModule.invoke;
	}
} catch {
	// Tauri not available
}

// Store for Tauri file references (since we can't get real paths from drag-and-drop)
const tauriFileStore = new Map<string, File>();
let tauriFileCounter = 0;

// Helper function to get path for file (Electron or Tauri)
function getLocalPathForFile(file: File): string | null {
	// Electron: native file path available via (file as any).path
	if ((file as any).path) {
		return (file as any).path;
	}
	
	// Tauri: Try to get file path via invoke
	// Note: In Tauri, the File object from drag-and-drop may not have a direct path
	// We need to use a different approach - store file reference and use blob URL
	if (tauriInvoke && file.name) {
		// For Tauri, we cannot directly get the file path from a dragged File object
		// The File object comes from the browser's DataTransfer API which doesn't expose real paths
		// Return null to trigger the blob URL fallback in extractEditorsDropData
		return null;
	}
	
	return null;
}

//#region Editor / Resources DND

export const CodeDataTransfers = {
	EDITORS: 'CodeEditors',
	FILES: 'CodeFiles',
	SYMBOLS: 'application/vnd.code.symbols',
	MARKERS: 'application/vnd.code.diagnostics',
	NOTEBOOK_CELL_OUTPUT: 'notebook-cell-output',
	SCM_HISTORY_ITEM: 'scm-history-item'
};

export interface IDraggedResourceEditorInput extends IBaseTextResourceEditorInput {
	resource: URI | undefined;

	/**
	 * A hint that the source of the dragged editor input
	 * might not be the application but some external tool.
	 */
	isExternal?: boolean;

	/**
	 * Whether we probe for the dropped editor to be a workspace
	 * (i.e. code-workspace file or even a folder), allowing to
	 * open it as workspace instead of opening as editor.
	 */
	allowWorkspaceOpen?: boolean;
}

export function extractEditorsDropData(e: DragEvent): Array<IDraggedResourceEditorInput> {
	console.log('[DND] extractEditorsDropData called');
	const editors: IDraggedResourceEditorInput[] = [];
	if (e.dataTransfer && e.dataTransfer.types.length > 0) {
		console.log('[DND] dataTransfer types:', e.dataTransfer.types);
		console.log('[DND] dataTransfer files count:', e.dataTransfer.files?.length || 0);
		
		// Data Transfer: Code Editors
		const rawEditorsData = e.dataTransfer.getData(CodeDataTransfers.EDITORS);
		if (rawEditorsData) {
			try {
				editors.push(...parse(rawEditorsData));
			} catch (error) {
				// Invalid transfer
			}
		}

		// Data Transfer: Resources
		else {
			try {
				const rawResourcesData = e.dataTransfer.getData(DataTransfers.RESOURCES);
				editors.push(...createDraggedEditorInputFromRawResourcesData(rawResourcesData));
			} catch (error) {
				// Invalid transfer
			}
		}

		// Check for native file transfer (Electron or Tauri)
		if (e.dataTransfer?.files) {
			console.log('[DND] Processing', e.dataTransfer.files.length, 'files from dataTransfer');
			for (let i = 0; i < e.dataTransfer.files.length; i++) {
				const file = e.dataTransfer.files[i];
				console.log('[DND] File', i, ':', file.name, 'type:', file.type, 'size:', file.size);
				const filePath = getLocalPathForFile(file);
				console.log('[DND] File path result:', filePath);
				
				// Check if this is actually a directory (via webkitRelativePath or size === 0 and name suggests folder)
				const isDirectory = file.name.endsWith('/') || (file.size === 0 && !file.type && file.webkitRelativePath?.includes('/'));
				
				if (file && filePath) {
					try {
						console.log('[DND] Adding editor with file path:', filePath, 'isDirectory:', isDirectory);
						editors.push({ resource: URI.file(filePath), isExternal: true, allowWorkspaceOpen: true });
					} catch (error) {
						console.error('[DND] Error creating URI from file path:', error);
						// Invalid URI
					}
					} else if (file && !filePath && tauriInvoke) {
						// Tauri: For files without a direct path, store the file reference
						// and use a virtual path scheme that we can resolve later
						try {
							console.log('[DND] Storing Tauri file reference:', file.name, 'isDirectory:', isDirectory);
							// Store file in our local store with a unique ID
							const fileId = `tauri-file-${++tauriFileCounter}`;
							tauriFileStore.set(fileId, file);
							console.log('[DND] File stored with ID:', fileId);
							// Create a virtual URI that encodes the file ID
							editors.push({ 
								resource: URI.parse(`tauri-file:${encodeURIComponent(fileId)}/${encodeURIComponent(file.name)}`), 
								isExternal: true, 
								allowWorkspaceOpen: isDirectory // Allow workspace open for directories 
							});
						} catch (error) {
							console.error('[DND] Error storing Tauri file reference:', error);
							// Failed to store file reference
						}
					} else if (file) {
						console.log('[DND] File has no path and tauriInvoke not available');
				}
			}
		}

		// Check for CodeFiles transfer
		const rawCodeFiles = e.dataTransfer.getData(CodeDataTransfers.FILES);
		if (rawCodeFiles) {
			try {
				const codeFiles: string[] = JSON.parse(rawCodeFiles);
				for (const codeFile of codeFiles) {
					editors.push({ resource: URI.file(codeFile), isExternal: true, allowWorkspaceOpen: true });
				}
			} catch (error) {
				// Invalid transfer
			}
		}

		// Workbench contributions
		const contributions = Registry.as<IDragAndDropContributionRegistry>(Extensions.DragAndDropContribution).getAll();
		for (const contribution of contributions) {
			const data = e.dataTransfer.getData(contribution.dataFormatKey);
			if (data) {
				try {
					editors.push(...contribution.getEditorInputs(data));
				} catch (error) {
					// Invalid transfer
				}
			}
		}
	}

	// Prevent duplicates: it is possible that we end up with the same
	// dragged editor multiple times because multiple data transfers
	// are being used (https://github.com/microsoft/vscode/issues/128925)

	const coalescedEditors: IDraggedResourceEditorInput[] = [];
	const seen = new ResourceMap<boolean>();
	for (const editor of editors) {
		if (!editor.resource) {
			coalescedEditors.push(editor);
		} else if (!seen.has(editor.resource)) {
			coalescedEditors.push(editor);
			seen.set(editor.resource, true);
		}
	}

	return coalescedEditors;
}

export async function extractEditorsAndFilesDropData(
	accessor: ServicesAccessor,
	e: DragEvent
): Promise<Array<IDraggedResourceEditorInput>> {
	console.log('[DND] extractEditorsAndFilesDropData called');
	const editors = extractEditorsDropData(e);
	console.log('[DND] extractEditorsDropData returned', editors.length, 'editors');

	// Web: Check for file transfer
	if (e.dataTransfer && isWeb && containsDragType(e, DataTransfers.FILES)) {
		console.log('[DND] Detected web file transfer');
		const files = e.dataTransfer.items;
		if (files) {
			const instantiationService = accessor.get(IInstantiationService);
			const filesData = await instantiationService.invokeFunction(accessor => extractFilesDropData(accessor, e));
			console.log('[DND] extractFilesDropData returned', filesData.length, 'files');
			for (const fileData of filesData) {
				editors.push({
					resource: fileData.resource,
					contents: fileData.contents?.toString(),
					isExternal: true,
					allowWorkspaceOpen: fileData.isDirectory
				});
			}
		}
	}

	console.log('[DND] extractEditorsAndFilesDropData returning', editors.length, 'total editors');
	return editors;
}

export function createDraggedEditorInputFromRawResourcesData(
	rawResourcesData: string | undefined
): IDraggedResourceEditorInput[] {
	const editors: IDraggedResourceEditorInput[] = [];

	if (rawResourcesData) {
		const resourcesRaw: string[] = JSON.parse(rawResourcesData);
		for (const resourceRaw of resourcesRaw) {
			if (resourceRaw.indexOf(':') > 0) {
				// mitigate https://github.com/microsoft/vscode/issues/124946
				const { selection, uri } = extractSelection(URI.parse(resourceRaw));
				editors.push({ resource: uri, options: { selection } });
			}
		}
	}

	return editors;
}

interface IFileTransferData {
	resource: URI;
	isDirectory?: boolean;
	contents?: VSBuffer;
}

async function extractFilesDropData(accessor: ServicesAccessor, event: DragEvent): Promise<IFileTransferData[]> {
	// Try to extract via `FileSystemHandle`
	if (WebFileSystemAccess.supported(mainWindow)) {
		const items = event.dataTransfer?.items;
		if (items) {
			return extractFileTransferData(accessor, items);
		}
	}

	// Try to extract via `FileList`
	const files = event.dataTransfer?.files;
	if (!files) {
		return [];
	}

	return extractFileListData(accessor, files);
}

async function extractFileTransferData(
	accessor: ServicesAccessor,
	items: DataTransferItemList
): Promise<IFileTransferData[]> {
	const fileSystemProvider = accessor.get(IFileService).getProvider(Schemas.file);

	if (!(fileSystemProvider instanceof HTMLFileSystemProvider)) {
		return []; // only supported when running in web
	}

	const results: DeferredPromise<IFileTransferData | undefined>[] = [];

	for (let i = 0; i < items.length; i++) {
		const file = items[i];
		if (file) {
			const result = new DeferredPromise<IFileTransferData | undefined>();
			results.push(result);

			(async () => {
				try {
					const handle = await file.getAsFileSystemHandle();
					if (!handle) {
						result.complete(undefined);
						return;
					}

					if (WebFileSystemAccess.isFileSystemFileHandle(handle)) {
						result.complete({
							resource: await fileSystemProvider.registerFileHandle(handle),
							isDirectory: false
						});
					} else if (WebFileSystemAccess.isFileSystemDirectoryHandle(handle)) {
						result.complete({
							resource: await fileSystemProvider.registerDirectoryHandle(handle),
							isDirectory: true
						});
					} else {
						result.complete(undefined);
					}
				} catch (error) {
					result.complete(undefined);
				}
			})();
		}
	}

	return coalesce(await Promise.all(results.map(result => result.p)));
}

export async function extractFileListData(accessor: ServicesAccessor, files: FileList): Promise<IFileTransferData[]> {
	const dialogService = accessor.get(IDialogService);

	const results: DeferredPromise<IFileTransferData | undefined>[] = [];

	for (let i = 0; i < files.length; i++) {
		const file = files.item(i);
		if (file) {
			// Skip for very large files because this operation is unbuffered
			if (file.size > 100 * ByteSize.MB) {
				dialogService.warn(
					localize(
						'fileTooLarge',
						'File is too large to open as untitled editor. Please upload it first into the file explorer and then try again.'
					)
				);
				continue;
			}

			const result = new DeferredPromise<IFileTransferData | undefined>();
			results.push(result);

			const reader = new FileReader();

			reader.onerror = () => result.complete(undefined);
			reader.onabort = () => result.complete(undefined);

			reader.onload = async event => {
				const name = file.name;
				const loadResult = event.target?.result ?? undefined;
				if (typeof name !== 'string' || typeof loadResult === 'undefined') {
					result.complete(undefined);
					return;
				}

				result.complete({
					resource: URI.from({ scheme: Schemas.untitled, path: name }),
					contents:
						typeof loadResult === 'string' ? VSBuffer.fromString(loadResult) : VSBuffer.wrap(new Uint8Array(loadResult))
				});
			};

			// Start reading
			reader.readAsArrayBuffer(file);
		}
	}

	return coalesce(await Promise.all(results.map(result => result.p)));
}

//#endregion

export function containsDragType(event: DragEvent, ...dragTypesToFind: string[]): boolean {
	if (!event.dataTransfer) {
		return false;
	}

	const dragTypes = event.dataTransfer.types;
	const lowercaseDragTypes: string[] = [];
	for (let i = 0; i < dragTypes.length; i++) {
		lowercaseDragTypes.push(dragTypes[i].toLowerCase()); // somehow the types are lowercase
	}

	for (const dragType of dragTypesToFind) {
		if (lowercaseDragTypes.indexOf(dragType.toLowerCase()) >= 0) {
			return true;
		}
	}

	return false;
}

//#region DND contributions

export interface IResourceStat {
	readonly resource: URI;
	readonly isDirectory?: boolean;
	readonly selection?: ITextEditorSelection;
}

export interface IResourceDropHandler {
	/**
	 * Handle a dropped resource.
	 * @param resource The resource that was dropped
	 * @param accessor Service accessor to get services
	 * @returns true if handled, false otherwise
	 */
	handleDrop(resource: URI, accessor: ServicesAccessor): Promise<boolean>;
}

export interface IDragAndDropContributionRegistry {
	/**
	 * Registers a drag and drop contribution.
	 */
	register(contribution: IDragAndDropContribution): void;

	/**
	 * Returns all registered drag and drop contributions.
	 */
	getAll(): IterableIterator<IDragAndDropContribution>;

	/**
	 * Register a handler for dropped resources.
	 * @returns A disposable that unregisters the handler when disposed
	 */
	registerDropHandler(handler: IResourceDropHandler): IDisposable;

	/**
	 * Handle a dropped resource using registered handlers.
	 * @param resource The resource that was dropped
	 * @param accessor Service accessor to get services
	 * @returns true if any handler handled the resource, false otherwise
	 */
	handleResourceDrop(resource: URI, accessor: ServicesAccessor): Promise<boolean>;
}

interface IDragAndDropContribution {
	readonly dataFormatKey: string;
	getEditorInputs(data: string): IDraggedResourceEditorInput[];
	setData(resources: IResourceStat[], event: DragMouseEvent | DragEvent): void;
}

class DragAndDropContributionRegistry implements IDragAndDropContributionRegistry {
	private readonly _contributions = new Map<string, IDragAndDropContribution>();
	private readonly _dropHandlers = new Set<IResourceDropHandler>();

	register(contribution: IDragAndDropContribution): void {
		if (this._contributions.has(contribution.dataFormatKey)) {
			throw new Error(`A drag and drop contributiont with key '${contribution.dataFormatKey}' was already registered.`);
		}
		this._contributions.set(contribution.dataFormatKey, contribution);
	}

	getAll(): IterableIterator<IDragAndDropContribution> {
		return this._contributions.values();
	}

	registerDropHandler(handler: IResourceDropHandler): IDisposable {
		this._dropHandlers.add(handler);
		return toDisposable(() => this._dropHandlers.delete(handler));
	}

	async handleResourceDrop(resource: URI, accessor: ServicesAccessor): Promise<boolean> {
		for (const handler of this._dropHandlers) {
			if (await handler.handleDrop(resource, accessor)) {
				return true;
			}
		}
		return false;
	}
}

export const Extensions = {
	DragAndDropContribution: 'workbench.contributions.dragAndDrop'
};

Registry.add(Extensions.DragAndDropContribution, new DragAndDropContributionRegistry());

//#endregion

//#region DND Utilities

/**
 * A singleton to store transfer data during drag & drop operations that are only valid within the application.
 */
export class LocalSelectionTransfer<T> {
	private static readonly INSTANCE = new LocalSelectionTransfer();

	private data?: T[];
	private proto?: T;

	private constructor() {
		// protect against external instantiation
	}

	static getInstance<T>(): LocalSelectionTransfer<T> {
		return LocalSelectionTransfer.INSTANCE as LocalSelectionTransfer<T>;
	}

	hasData(proto: T): boolean {
		return proto && proto === this.proto;
	}

	clearData(proto: T): void {
		if (this.hasData(proto)) {
			this.proto = undefined;
			this.data = undefined;
		}
	}

	getData(proto: T): T[] | undefined {
		if (this.hasData(proto)) {
			return this.data;
		}

		return undefined;
	}

	setData(data: T[], proto: T): void {
		if (proto) {
			this.data = data;
			this.proto = proto;
		}
	}
}

export interface DocumentSymbolTransferData {
	name: string;
	fsPath: string;
	range: {
		startLineNumber: number;
		startColumn: number;
		endLineNumber: number;
		endColumn: number;
	};
	kind: number;
}

export interface NotebookCellOutputTransferData {
	outputId: string;
}

function setDataAsJSON(e: DragEvent, kind: string, data: unknown) {
	e.dataTransfer?.setData(kind, JSON.stringify(data));
}

function getDataAsJSON<T>(e: DragEvent, kind: string, defaultValue: T): T {
	const rawSymbolsData = e.dataTransfer?.getData(kind);
	if (rawSymbolsData) {
		try {
			return JSON.parse(rawSymbolsData);
		} catch (error) {
			// Invalid transfer
		}
	}

	return defaultValue;
}

export function extractSymbolDropData(e: DragEvent): DocumentSymbolTransferData[] {
	return getDataAsJSON(e, CodeDataTransfers.SYMBOLS, []);
}

export function fillInSymbolsDragData(symbolsData: readonly DocumentSymbolTransferData[], e: DragEvent): void {
	setDataAsJSON(e, CodeDataTransfers.SYMBOLS, symbolsData);
}

export type MarkerTransferData = IMarker | { uri: UriComponents };

export function extractMarkerDropData(e: DragEvent): MarkerTransferData[] | undefined {
	return getDataAsJSON(e, CodeDataTransfers.MARKERS, undefined);
}

export function fillInMarkersDragData(markerData: MarkerTransferData[], e: DragEvent): void {
	setDataAsJSON(e, CodeDataTransfers.MARKERS, markerData);
}

export function extractNotebookCellOutputDropData(e: DragEvent): NotebookCellOutputTransferData | undefined {
	return getDataAsJSON(e, CodeDataTransfers.NOTEBOOK_CELL_OUTPUT, undefined);
}

interface IElectronWebUtils {
	vscode?: {
		webUtils?: {
			getPathForFile(file: File): string;
		};
	};
}

/**
 * A helper to get access to Electrons `webUtils.getPathForFile` function
 * in a safe way without crashing the application when running in the web.
 * For Tauri, we use a custom command to get the file path.
 */
export function getPathForFile(file: File): string | undefined {
	if (isNative && typeof (globalThis as IElectronWebUtils).vscode?.webUtils?.getPathForFile === 'function') {
		return (globalThis as IElectronWebUtils).vscode?.webUtils?.getPathForFile(file);
	}

	// Tauri: Use invoke to get file path
	if (tauriInvoke) {
		try {
			// Create a temporary object with file info that can be sent to Rust
			const fileInfo = { name: file.name, size: file.size, type: file.type };
			// We need to use a different approach for Tauri - store file reference and return a virtual path
			// For now, return undefined and let the file upload mechanism handle it
			return undefined;
		} catch {
			return undefined;
		}
	}

	return undefined;
}

//#endregion
