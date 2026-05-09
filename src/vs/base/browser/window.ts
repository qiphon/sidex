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

// Cache for mock document to avoid infinite recursion
let mockDocument: any = null;

if (typeof fallbackWindow.document !== 'object') {
	// Create mock element factory that captures document reference
	const createMockElement = (tag: string, ns?: string) => {
		const children: any[] = [];
		let idValue = '';
		let classNameValue = '';
		const el: any = {
			tagName: tag.toUpperCase(),
			namespaceURI: ns,
			ownerDocument: mockDocument,
			get id() { return idValue; },
			set id(val: string) { idValue = val; },
			get className() { return classNameValue; },
			set className(val: string) { classNameValue = val; },
			setAttribute: () => {},
			removeAttribute: () => {},
			addEventListener: () => {},
			removeEventListener: () => {},
			appendChild: (node: any) => { children.push(node); return node; },
			removeChild: () => {},
			insertBefore: () => {},
			replaceChild: () => {},
			append: (...nodes: any[]) => { nodes.forEach(n => children.push(n)); },
			prepend: (...nodes: any[]) => { nodes.forEach(n => children.unshift(n)); },
			remove: () => {},
			classList: { add: () => {}, remove: () => {}, toggle: () => {} },
			style: {},
			children: children,
			childNodes: children,
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
		};
		return el;
	};

	// Create document first, then populate it
	mockDocument = {
		defaultView: fallbackWindow,
		hasFocus: () => false,
		activeElement: null,
		addEventListener: () => {},
		removeEventListener: () => {},
		createElement: (tag: string) => createMockElement(tag),
		createElementNS: (ns: string, tag: string) => createMockElement(tag, ns),
		createTextNode: (text: string) => ({
			textContent: text,
			nodeType: 3,
			ownerDocument: mockDocument,
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
			ownerDocument: mockDocument
		}),
		querySelector: () => null,
		querySelectorAll: () => [],
		getElementById: () => null,
		getElementsByClassName: () => [],
		getElementsByTagName: () => [],
		body: null as any,
		documentElement: null as any,
		head: null as any
	};

	// Now create body, documentElement, head with reference to mockDocument
	mockDocument.body = createMockElement('body');
	mockDocument.documentElement = createMockElement('html');
	mockDocument.head = createMockElement('head');

	Object.defineProperty(fallbackWindow, 'document', {
		get: () => mockDocument
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
