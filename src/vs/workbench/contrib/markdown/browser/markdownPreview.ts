/*---------------------------------------------------------------------------------------------
 *  Copyright (c) Microsoft Corporation. All rights reserved.
 *  Licensed under the MIT License. See License.txt in the project root for license information.
 *--------------------------------------------------------------------------------------------*/

import { Disposable, DisposableStore } from '../../../../base/common/lifecycle.js';
import { URI } from '../../../../base/common/uri.js';
import { IEditorService, SIDE_GROUP } from '../../../services/editor/common/editorService.js';
import { IWebviewWorkbenchService } from '../../webviewPanel/browser/webviewWorkbenchService.js';
import { WebviewInput } from '../../webviewPanel/browser/webviewEditorInput.js';
import { WebviewInitInfo } from '../../webview/browser/webview.js';
import { asWebviewUri } from '../../webview/common/webview.js';
import { EditorInput } from '../../../common/editor/editorInput.js';
import { ILogService } from '../../../../platform/log/common/log.js';
import { IStorageService, StorageScope, StorageTarget } from '../../../../platform/storage/common/storage.js';
import { ITextFileService } from '../../../services/textfile/common/textfiles.js';
import { DEFAULT_MARKDOWN_STYLES } from './markdownDocumentRenderer.js';
import * as marked from '../../../../base/common/marked/marked.js';

const MARKDOWN_PREVIEW_VIEW_TYPE = 'sidex.markdown.preview';
const PREVIEW_OPEN_FILES_KEY = 'markdown.preview.openFiles';

const HAS_SCHEME = /^\w[\w\d+.-]*:/;

function resolveMarkdownUri(baseDir: URI, href: string): URI {
	if (HAS_SCHEME.test(href)) {
		return URI.parse(href);
	}
	return URI.joinPath(baseDir, href);
}

function rewriteImageSrcs(html: string, baseDir: URI): string {
	return html.replace(
		/<img\s+([^>]*?)src="([^"]*?)"([^>]*)>/gi,
		(_match, before: string, src: string, after: string) => {
			if (HAS_SCHEME.test(src) || src.startsWith('data:')) {
				return _match;
			}
			const resolved = resolveMarkdownUri(baseDir, src);
			const webviewSafe = asWebviewUri(resolved);
			return `<img ${before}src="${webviewSafe.toString()}"${after}>`;
		}
	);
}

export class MarkdownPreviewManager extends Disposable {
	private _webviewInput: WebviewInput | undefined;
	private readonly _previewDisposables = this._register(new DisposableStore());
	private _debounceTimer: ReturnType<typeof setTimeout> | undefined;
	private _currentResource: URI | undefined;

	constructor(
		@IEditorService private readonly _editorService: IEditorService,
		@IWebviewWorkbenchService private readonly _webviewWorkbenchService: IWebviewWorkbenchService,
		@ITextFileService private readonly _textFileService: ITextFileService,
		@ILogService private readonly _logService: ILogService,
		@IStorageService private readonly _storageService: IStorageService
	) {
		super();
	}

	showPreview(side?: boolean): void {
		const editor = this._editorService.activeEditor;
		if (!editor) {
			this._logService.warn('[Markdown Preview] No active editor');
			return;
		}

		const resource = editor instanceof EditorInput ? editor.resource : undefined;
		if (!resource || !resource.path.endsWith('.md')) {
			this._logService.warn('[Markdown Preview] Active editor is not a markdown file');
			return;
		}

		this._currentResource = resource;
		this._createOrUpdateWebview(resource, side);
	}

	static readonly ID = 'workbench.contrib.markdownPreview';

	toggle(): void {
		if (this._webviewInput) {
			this._closePreview();
		} else {
			this.showPreview();
		}
	}

	hasActivePreview(): boolean {
		return this._webviewInput !== undefined && !this._webviewInput.isDisposed();
	}

	static wasPreviewOpen(storageService: IStorageService, resource: URI): boolean {
		const openFiles: string[] = JSON.parse(storageService.get(PREVIEW_OPEN_FILES_KEY, StorageScope.WORKSPACE, '[]'));
		return openFiles.includes(resource.toString());
	}

	private _createOrUpdateWebview(resource: URI, side?: boolean): void {
		const title = `Preview: ${resource.path.split('/').pop() || resource.path}`;
		const parentDir = URI.joinPath(resource, '..');

		if (!this._webviewInput) {
			const initInfo: WebviewInitInfo = {
				title,
				options: {
					enableFindWidget: true,
					retainContextWhenHidden: true
				},
				contentOptions: {
					allowScripts: false,
					localResourceRoots: [parentDir]
				},
				extension: undefined
			};

			this._webviewInput = this._webviewWorkbenchService.openWebview(
				initInfo,
				MARKDOWN_PREVIEW_VIEW_TYPE,
				title,
				undefined,
				{ preserveFocus: true, group: side ? SIDE_GROUP : undefined }
			);

			this._register(
				this._webviewInput.onWillDispose(() => {
					this._clearDebounceTimer();
					this._webviewInput = undefined;
					this._previewDisposables.clear();
					this._currentResource = undefined;
				})
			);

			const openFiles: string[] = JSON.parse(
				this._storageService.get(PREVIEW_OPEN_FILES_KEY, StorageScope.WORKSPACE, '[]')
			);
			if (!openFiles.includes(resource.toString())) {
				openFiles.push(resource.toString());
				this._storageService.store(PREVIEW_OPEN_FILES_KEY, openFiles, StorageScope.WORKSPACE, StorageTarget.MACHINE);
			}
		} else {
			this._webviewInput.setWebviewTitle(title);
		}

		this._renderContent();
		this._listenToContentChanges();
	}

	private _listenToContentChanges(): void {
		this._previewDisposables.clear();

		if (!this._currentResource) {
			return;
		}

		const fileModel = this._textFileService.files.get(this._currentResource);
		if (!fileModel) {
			return;
		}

		if (!fileModel.isResolved()) {
			this._previewDisposables.add(
				this._textFileService.files.onDidResolve(e => {
					if (this._currentResource && e.model.resource.toString() === this._currentResource.toString()) {
						this._listenToContentChanges();
						this._renderContent();
					}
				})
			);
			return;
		}

		const textModel = fileModel.textEditorModel;
		this._previewDisposables.add(
			textModel.onDidChangeContent(() => {
				if (this._debounceTimer) {
					clearTimeout(this._debounceTimer);
				}
				this._debounceTimer = setTimeout(() => {
					this._renderContent();
				}, 300);
			})
		);
	}

	private _renderContent(): void {
		if (!this._webviewInput || !this._currentResource) {
			return;
		}

		const fileModel = this._textFileService.files.get(this._currentResource);
		if (!fileModel?.isResolved()) {
			return;
		}

		const baseDir = URI.joinPath(this._currentResource, '..');
		const textModel = fileModel.textEditorModel;
		const text = textModel.getValue();
		const parsedHtml = new marked.Marked().parse(text) as string;
		const htmlBody = rewriteImageSrcs(parsedHtml, baseDir);

		const html = `<!DOCTYPE html>
<html>
<head>
	<meta charset="UTF-8">
	<meta name="viewport" content="width=device-width, initial-scale=1.0">
	<style>
		:root {
			--vscode-editor-foreground: #d4d4d4;
			--vscode-editor-font-family: "SF Mono", Monaco, Menlo, Consolas, "Ubuntu Mono", "Liberation Mono", "DejaVu Sans Mono", "Courier New", monospace;
			--vscode-editor-font-weight: normal;
			--vscode-editor-font-size: 14px;
			--vscode-textCodeBlock-background: rgba(127, 127, 127, 0.1);
			--text-link-decoration: none;
		}
		body {
			background-color: #1e1e1e;
			color: #d4d4d4;
		}
		pre {
			background-color: var(--vscode-textCodeBlock-background);
		}
	</style>
	<style>${DEFAULT_MARKDOWN_STYLES}</style>
</head>
<body>${htmlBody}</body>
</html>`;

		this._webviewInput.webview.setHtml(html);
	}

	private _clearDebounceTimer(): void {
		if (this._debounceTimer) {
			clearTimeout(this._debounceTimer);
			this._debounceTimer = undefined;
		}
	}

	private _closePreview(): void {
		if (this._webviewInput) {
			if (this._currentResource) {
				const openFiles: string[] = JSON.parse(
					this._storageService.get(PREVIEW_OPEN_FILES_KEY, StorageScope.WORKSPACE, '[]')
				);
				const filtered = openFiles.filter((f: string) => f !== this._currentResource!.toString());
				this._storageService.store(PREVIEW_OPEN_FILES_KEY, filtered, StorageScope.WORKSPACE, StorageTarget.MACHINE);
			}
			this._clearDebounceTimer();
			this._webviewInput.dispose();
			this._webviewInput = undefined;
			this._previewDisposables.clear();
			this._currentResource = undefined;
		}
	}
}
