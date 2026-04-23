type Translations = Record<string, string>;

const sidexTranslations: Record<string, Translations> = {
	'zh-cn': {
		'remote.pick.placeholder': '选择一个选项来打开远程窗口',
		'remote.pick.connectToTunnel': '连接到隧道…',
		'remote.pick.tunnelsProvider': '远程隧道',
		'remote.pick.connectToHost': '连接到主机…',
		'remote.pick.sshProvider': '远程 SSH',
		'remote.pick.wsl': '连接到 WSL…',
		'remote.pick.wslProvider': '远程 WSL',
		'remote.pick.container': '在容器中打开文件夹…',
		'remote.pick.containerProvider': '开发容器',
		'remote.pick.codespace': '连接到 Codespace…',
		'remote.pick.codespaceProvider': 'GitHub Codespaces',
		'remote.codespace.tokenPrompt': '具有 codespace 范围的 GitHub 个人访问令牌',
		'remote.codespace.none': '未找到此帐户的 Codespace。',
		'remote.codespace.pick': '选择一个 Codespace',
		'remote.codespace.connected': '已连接到 Codespace: {0}',
		'remote.codespace.failed': 'Codespace 连接失败: {0}',
		'explorerViewlet.cloneRepository': '你可以在本地克隆一个仓库。\n{0}',
		remoteExplorer: '远程资源管理器',
		remoteExplorerViewIcon: '远程资源管理器视图图标。',
		'sidex.remote.refresh': '刷新远程资源管理器',
		'sidex.remote.openExplorer': '远程资源管理器',
		'remote.signInMicrosoft': '使用 Microsoft 登录',
		'remote.signInGitHub': '使用 GitHub 登录',
		'remote.ssh.noHosts': '未在 ~/.ssh/config 中找到 SSH 目标',
		'remote.ssh.empty': '~/.ssh/config 中没有 SSH 目标 — 在下方添加',
		'remote.codespaces.empty': '未找到 Codespace',
		'remote.containers.noContainers': '未找到运行中的容器',
		'remote.section.tunnels': '隧道',
		'remote.section.ssh': 'SSH',
		'remote.section.codespaces': 'GitHub Codespaces',
		'remote.section.containers': '开发容器',
		'remote.signIn': '登录',
		'remote.connect': '连接',
		'remote.tunnels': '隧道',
		'remote.ssh': 'SSH',
		'remote.codespaces': 'GitHub Codespaces',
		'remote.containers': '开发容器',
		'remote.wsl': 'WSL 目标',
		'remote.codespaces.signIn': '使用 GitHub 登录以查看你的 Codespace'
	},
	ja: {
		'remote.pick.placeholder': 'リモートウィンドウを開くオプションを選択',
		'remote.pick.connectToTunnel': 'トンネルに接続…',
		'remote.pick.connectToHost': 'ホストに接続…',
		'remote.pick.wsl': 'WSL に接続…',
		'remote.pick.container': 'コンテナーでフォルダーを開く…',
		'remote.pick.codespace': 'Codespace に接続…',
		remoteExplorer: 'リモート エクスプローラー'
	},
	ko: {
		'remote.pick.placeholder': '원격 창을 여는 옵션 선택',
		'remote.pick.connectToTunnel': '터널에 연결…',
		'remote.pick.connectToHost': '호스트에 연결…',
		'remote.pick.wsl': 'WSL에 연결…',
		'remote.pick.container': '컨테이너에서 폴더 열기…',
		'remote.pick.codespace': 'Codespace에 연결…',
		remoteExplorer: '원격 탐색기'
	}
};

export async function loadNlsMessages(): Promise<void> {
	const locale = localStorage.getItem('vscode.nls.locale');
	console.log('[SideX NLS] locale from localStorage:', locale);

	if (!locale || locale.toLowerCase().startsWith('en')) {
		return;
	}

	let extensionId = localStorage.getItem('vscode.nls.languagePackExtensionId');
	console.log('[SideX NLS] extensionId from localStorage:', extensionId);

	if (!extensionId) {
		extensionId = await detectInstalledLanguagePack(locale);
		if (extensionId) {
			localStorage.setItem('vscode.nls.languagePackExtensionId', extensionId);
			console.log('[SideX NLS] auto-detected language pack:', extensionId);
		} else {
			console.warn('[SideX NLS] No language pack found for locale:', locale);
			return;
		}
	}

	try {
		const translations = (await loadFromDisk(extensionId)) ?? (await loadFromGallery(extensionId));

		if (!translations) {
			console.warn('[SideX NLS] No translations found for', extensionId);
			return;
		}

		// Merge SideX-specific translations for strings not in the VS Code language pack
		const sidexExtra = sidexTranslations[locale.toLowerCase()];
		if (sidexExtra) {
			for (const [key, val] of Object.entries(sidexExtra)) {
				if (!(key in translations)) {
					translations[key] = val;
				}
			}
		}

		const indexRes = await fetch('/nls.messages.json');
		if (indexRes.ok) {
			const contentType = indexRes.headers.get('content-type') ?? '';
			if (contentType.includes('json')) {
				const nlsEntries: Array<{ key: string; msg: string }> = await indexRes.json();
				if (nlsEntries.length > 0) {
					(globalThis as any)._VSCODE_NLS_MESSAGES = nlsEntries.map(({ key, msg }) => translations[key] ?? msg);
					(globalThis as any)._VSCODE_NLS_LANGUAGE = locale;
					console.log(`[SideX NLS] Loaded ${nlsEntries.length} translations for ${locale} (indexed mode)`);
					return;
				}
			}
		}

		(globalThis as any)._VSCODE_NLS_TRANSLATIONS = translations;
		(globalThis as any)._VSCODE_NLS_LANGUAGE = locale;
		console.log(`[SideX NLS] Loaded ${Object.keys(translations).length} translations for ${locale} (key mode)`);
	} catch (e) {
		console.warn('[SideX NLS] Failed to load translations:', e);
	}
}

async function loadFromDisk(extensionId: string): Promise<Translations | null> {
	try {
		const { invoke } = await import('@tauri-apps/api/core');

		const homedir: string | null =
			(await invoke<string>('get_env', { key: 'HOME' }).catch(() => null)) ??
			(await invoke<string>('get_env', { key: 'USERPROFILE' }).catch(() => null));

		if (!homedir) {
			return null;
		}

		const path = `${homedir}/.sidex/extensions/${extensionId}/translations/main.i18n.json`;
		const raw = await invoke<string>('read_file', { path });
		const result = parseBundle(raw);
		if (result) {
			console.log(`[SideX NLS] Loaded translations from disk: ${path}`);
		}
		return result;
	} catch {
		return null;
	}
}

async function loadFromGallery(extensionId: string): Promise<Translations | null> {
	const [publisher, name] = extensionId.split('.');
	if (!publisher || !name) {
		return null;
	}

	const urls = [
		`https://marketplace.siden.ai/api/gallery/publishers/${publisher}/vsextensions/${name}/latest/vspackage`,
		`https://open-vsx.org/api/${publisher}/${name}/latest`
	];

	for (const metaUrl of urls) {
		try {
			const meta = await fetch(metaUrl).then(r => (r.ok ? r.json() : null));
			if (!meta?.version) {
				continue;
			}
			const translationUrl = `https://open-vsx.org/vscode/unpkg/${publisher}/${name}/${meta.version}/extension/translations/main.i18n.json`;
			const res = await fetch(translationUrl);
			if (res.ok) {
				const result = parseBundle(await res.text());
				if (result) {
					console.log(`[SideX NLS] Loaded translations from gallery`);
					return result;
				}
			}
		} catch {
			continue;
		}
	}
	return null;
}

function parseBundle(raw: string): Translations | null {
	try {
		const bundles = JSON.parse(raw)?.contents;
		if (!bundles) {
			return null;
		}
		const messages: Translations = {};
		for (const bundle of Object.values(bundles)) {
			Object.assign(messages, bundle);
		}
		return messages;
	} catch {
		return null;
	}
}

const localeToPackName: Record<string, string> = {
	'zh-cn': 'MS-CEINTL.vscode-language-pack-zh-hans',
	'zh-tw': 'MS-CEINTL.vscode-language-pack-zh-hant',
	ja: 'MS-CEINTL.vscode-language-pack-ja',
	ko: 'MS-CEINTL.vscode-language-pack-ko',
	de: 'MS-CEINTL.vscode-language-pack-de',
	fr: 'MS-CEINTL.vscode-language-pack-fr',
	es: 'MS-CEINTL.vscode-language-pack-es',
	it: 'MS-CEINTL.vscode-language-pack-it',
	'pt-br': 'MS-CEINTL.vscode-language-pack-pt-BR',
	ru: 'MS-CEINTL.vscode-language-pack-ru',
	tr: 'MS-CEINTL.vscode-language-pack-tr',
	pl: 'MS-CEINTL.vscode-language-pack-pl',
	cs: 'MS-CEINTL.vscode-language-pack-cs',
	hu: 'MS-CEINTL.vscode-language-pack-hu'
};

async function detectInstalledLanguagePack(locale: string): Promise<string | null> {
	const knownId = localeToPackName[locale.toLowerCase()];
	if (!knownId) {
		return null;
	}
	try {
		const { invoke } = await import('@tauri-apps/api/core');
		const homedir: string | null =
			(await invoke<string>('get_env', { key: 'HOME' }).catch(() => null)) ??
			(await invoke<string>('get_env', { key: 'USERPROFILE' }).catch(() => null));
		if (!homedir) {
			return null;
		}
		const path = `${homedir}/.sidex/extensions/${knownId}/translations/main.i18n.json`;
		await invoke<string>('read_file', { path });
		return knownId;
	} catch {
		return null;
	}
}
