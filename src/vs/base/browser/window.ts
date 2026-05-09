/*---------------------------------------------------------------------------------------------
 *  Copyright (c) Microsoft Corporation. All rights reserved.
 *  Licensed under the MIT License. See License.txt in the project root for license information.
 *--------------------------------------------------------------------------------------------*/

export type CodeWindow = Window &
	typeof globalThis & {
		readonly vscodeWindowId: number;
	};

export function ensureCodeWindow(targetWindow: Window, fallbackWindowId: number): asserts targetWindow is CodeWindow {
	const codeWindow = targetWindow as Partial<CodeWindow>;

	if (typeof codeWindow.vscodeWindowId !== 'number') {
		Object.defineProperty(codeWindow, 'vscodeWindowId', {
			get: () => fallbackWindowId
		});
	}
}

const fallbackWindow = globalThis as typeof globalThis & Partial<CodeWindow>;

if (typeof fallbackWindow.window !== 'object') {
	Object.defineProperty(fallbackWindow, 'window', {
		get: () => fallbackWindow
	});
}

if (typeof fallbackWindow.document !== 'object') {
	Object.defineProperty(fallbackWindow, 'document', {
		get: () => ({
			defaultView: fallbackWindow,
			hasFocus: () => false,
			activeElement: null,
			addEventListener: () => {},
			removeEventListener: () => {},
			createElement: (tag: string) => ({
				tagName: tag.toUpperCase(),
				ownerDocument: fallbackWindow.document,
				setAttribute: () => {},
				removeAttribute: () => {},
				addEventListener: () => {},
				removeEventListener: () => {},
				appendChild: () => {},
				removeChild: () => {},
				insertBefore: () => {},
				replaceChild: () => {},
				remove: () => {},
				classList: { add: () => {}, remove: () => {}, toggle: () => {} },
				style: {},
				children: [],
				childNodes: [],
				parentNode: null,
				nextSibling: null,
				previousSibling: null,
				firstChild: null,
				lastChild: null,
				textContent: '',
				innerHTML: '',
				offsetWidth: 0,
				offsetHeight: 0,
				clientWidth: 0,
				clientHeight: 0,
				getBoundingClientRect: () => ({ top: 0, left: 0, right: 0, bottom: 0, width: 0, height: 0, x: 0, y: 0 }),
				getAttribute: () => null,
				querySelector: () => null,
				querySelectorAll: () => [],
				dispatchEvent: () => true,
				contains: () => false
			}),
			createElementNS: (ns: string, tag: string) => ({
				tagName: tag.toUpperCase(),
				namespaceURI: ns,
				ownerDocument: fallbackWindow.document,
				setAttribute: () => {},
				removeAttribute: () => {},
				addEventListener: () => {},
				removeEventListener: () => {},
				appendChild: () => {},
				removeChild: () => {},
				insertBefore: () => {},
				replaceChild: () => {},
				remove: () => {},
				classList: { add: () => {}, remove: () => {}, toggle: () => {} },
				style: {},
				children: [],
				childNodes: [],
				parentNode: null,
				nextSibling: null,
				previousSibling: null,
				firstChild: null,
				lastChild: null,
				textContent: '',
				innerHTML: '',
				offsetWidth: 0,
				offsetHeight: 0,
				clientWidth: 0,
				clientHeight: 0,
				getBoundingClientRect: () => ({ top: 0, left: 0, right: 0, bottom: 0, width: 0, height: 0, x: 0, y: 0 }),
				getAttribute: () => null,
				querySelector: () => null,
				querySelectorAll: () => [],
				dispatchEvent: () => true,
				contains: () => false
			}),
			createTextNode: (text: string) => ({
				textContent: text,
				nodeType: 3,
				ownerDocument: fallbackWindow.document,
				parentNode: null,
				nextSibling: null,
				previousSibling: null
			}),
			createDocumentFragment: () => ({
				children: [],
				childNodes: [],
				appendChild: () => {},
				removeChild: () => {},
				insertBefore: () => {},
				replaceChild: () => {},
				ownerDocument: fallbackWindow.document
			}),
			querySelector: () => null,
			querySelectorAll: () => [],
			getElementById: () => null,
			getElementsByClassName: () => [],
			getElementsByTagName: () => [],
			body: {
				appendChild: () => {},
				removeChild: () => {},
				insertBefore: () => {},
				ownerDocument: fallbackWindow.document,
				children: [],
				classList: { add: () => {}, remove: () => {}, toggle: () => {} }
			},
			documentElement: {
				appendChild: () => {},
				removeChild: () => {},
				insertBefore: () => {},
				ownerDocument: fallbackWindow.document,
				children: [],
				classList: { add: () => {}, remove: () => {}, toggle: () => {} }
			},
			head: {
				appendChild: () => {},
				ownerDocument: fallbackWindow.document,
				children: []
			}
		})
	});
}

export const mainWindow = (typeof window === 'object' ? window : fallbackWindow) as CodeWindow;

export function isAuxiliaryWindow(obj: Window): obj is CodeWindow {
	if (obj === mainWindow) {
		return false;
	}

	const candidate = obj as CodeWindow | undefined;

	return typeof candidate?.vscodeWindowId === 'number';
}
