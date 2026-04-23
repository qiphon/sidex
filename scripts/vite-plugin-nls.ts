import type { Plugin } from 'vite';
import * as fs from 'fs';
import * as path from 'path';

interface NlsEntry {
	key: string;
	msg: string;
}

export function nlsPlugin(): Plugin {
	const entries: NlsEntry[] = [];
	const dedupIndex = new Map<string, number>();
	let isBuild = false;

	function getOrAddIndex(key: string, msg: string): number {
		const dedupKey = `${key}\0${msg}`;
		const existing = dedupIndex.get(dedupKey);
		if (existing !== undefined) {
			return existing;
		}
		const idx = entries.length;
		entries.push({ key, msg });
		dedupIndex.set(dedupKey, idx);
		return idx;
	}

	function prescanSourceFiles() {
		const srcDir = path.resolve(process.cwd(), 'src/vs');
		const files = walkDir(srcDir);
		let count = 0;
		for (const file of files) {
			if (!file.endsWith('.ts')) {
				continue;
			}
			const code = fs.readFileSync(file, 'utf-8');
			if (!code.includes('localize')) {
				continue;
			}
			const re = /\blocalize2?\s*\(/g;
			let m: RegExpExecArray | null;
			while ((m = re.exec(code)) !== null) {
				const argsStart = m.index + m[0].length;
				const firstArgEnd = findFirstArgEnd(code, argsStart);
				if (firstArgEnd < 0) {
					continue;
				}
				const key = extractKey(code.slice(argsStart, firstArgEnd).trim());
				if (!key) {
					continue;
				}
				const afterComma = skipWhitespace(code, firstArgEnd + 1);
				const strEnd = readStringLiteral(code, afterComma);
				if (strEnd < 0) {
					continue;
				}
				const msg = unquote(code.slice(afterComma, strEnd + 1));
				getOrAddIndex(key, msg);
				count++;
			}
		}
		console.log(
			`[vite-plugin-nls] Pre-scanned ${files.length} files, found ${count} NLS entries (${entries.length} unique)`
		);
	}

	function walkDir(dir: string): string[] {
		const results: string[] = [];
		for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
			const full = path.join(dir, entry.name);
			if (entry.isDirectory()) {
				results.push(...walkDir(full));
			} else {
				results.push(full);
			}
		}
		return results;
	}

	return {
		name: 'vite-plugin-nls',
		enforce: 'pre',

		config(_cfg, env) {
			isBuild = env.command === 'build';
		},

		configureServer(server) {
			server.middlewares.use('/nls.messages.json', (_req, res) => {
				res.setHeader('Content-Type', 'application/json');
				res.end(JSON.stringify(entries, null, 2));
			});
		},

		buildStart() {
			if (!isBuild) {
				prescanSourceFiles();
			}
		},

		transform(code, id) {
			if (!id.includes('/src/vs/') || !id.endsWith('.ts')) {
				return null;
			}
			if (!code.includes('localize')) {
				return null;
			}

			let result = '';
			let pos = 0;
			let didChange = false;

			const re = /\blocalize2?\s*\(/g;
			let m: RegExpExecArray | null;

			while ((m = re.exec(code)) !== null) {
				const argsStart = m.index + m[0].length;
				const firstArgEnd = findFirstArgEnd(code, argsStart);
				if (firstArgEnd < 0) {
					continue;
				}

				const key = extractKey(code.slice(argsStart, firstArgEnd).trim());
				if (!key) {
					continue;
				}

				const afterComma = skipWhitespace(code, firstArgEnd + 1);
				const strEnd = readStringLiteral(code, afterComma);
				if (strEnd < 0) {
					continue;
				}

				const msg = unquote(code.slice(afterComma, strEnd + 1));
				const idx = getOrAddIndex(key, msg);

				result += code.slice(pos, argsStart);
				result += String(idx);
				pos = firstArgEnd;
				didChange = true;
			}

			if (!didChange) {
				return null;
			}

			result += code.slice(pos);
			return { code: result };
		},

		generateBundle() {
			if (entries.length > 0) {
				this.emitFile({
					type: 'asset',
					fileName: 'nls.messages.json',
					source: JSON.stringify(entries, null, 2)
				});
			}
		}
	};
}

function extractKey(arg: string): string | null {
	if (arg.startsWith('{')) {
		const m = arg.match(/\bkey\s*:\s*(['"`])([^'"`]+)\1/);
		return m ? m[2] : null;
	}
	if ((arg.startsWith("'") || arg.startsWith('"') || arg.startsWith('`')) && arg.length > 2) {
		return arg.slice(1, -1);
	}
	return null;
}

function findFirstArgEnd(code: string, start: number): number {
	let depth = 0;
	let inStr: string | null = null;

	for (let i = start; i < code.length; i++) {
		const ch = code[i];

		if (inStr) {
			if (ch === '\\') {
				i++;
				continue;
			}
			if (ch === inStr) {
				inStr = null;
			}
			continue;
		}

		if (ch === '"' || ch === "'" || ch === '`') {
			inStr = ch;
			continue;
		}
		if (ch === '(' || ch === '{' || ch === '[') {
			depth++;
			continue;
		}
		if (ch === ')' || ch === '}' || ch === ']') {
			if (depth === 0) {
				return -1;
			}
			depth--;
			continue;
		}
		if (ch === ',' && depth === 0) {
			return i;
		}
	}
	return -1;
}

function skipWhitespace(code: string, pos: number): number {
	while (pos < code.length && /\s/.test(code[pos])) {
		pos++;
	}
	return pos;
}

function readStringLiteral(code: string, pos: number): number {
	const quote = code[pos];
	if (quote !== '"' && quote !== "'" && quote !== '`') {
		return -1;
	}
	for (let i = pos + 1; i < code.length; i++) {
		const ch = code[i];
		if (ch === '\\') {
			i++;
			continue;
		}
		if (ch === quote) {
			return i;
		}
	}
	return -1;
}

function unquote(literal: string): string {
	return literal
		.slice(1, -1)
		.replace(/\\'/g, "'")
		.replace(/\\"/g, '"')
		.replace(/\\n/g, '\n')
		.replace(/\\t/g, '\t')
		.replace(/\\\\/g, '\\');
}
