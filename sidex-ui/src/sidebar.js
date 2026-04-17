const EXT_ICON_MAP = {
  js: 'codicon-file-code',
  ts: 'codicon-file-code',
  jsx: 'codicon-file-code',
  tsx: 'codicon-file-code',
  rs: 'codicon-file-code',
  py: 'codicon-file-code',
  rb: 'codicon-file-code',
  go: 'codicon-file-code',
  c: 'codicon-file-code',
  cpp: 'codicon-file-code',
  h: 'codicon-file-code',
  java: 'codicon-file-code',
  sh: 'codicon-file-code',
  bash: 'codicon-file-code',
  zsh: 'codicon-file-code',
  css: 'codicon-file-code',
  scss: 'codicon-file-code',
  html: 'codicon-file-code',
  json: 'codicon-json',
  md: 'codicon-markdown',
  txt: 'codicon-file-text',
  toml: 'codicon-settings-gear',
  yaml: 'codicon-settings-gear',
  yml: 'codicon-settings-gear',
  lock: 'codicon-lock',
  svg: 'codicon-file-media',
  png: 'codicon-file-media',
  jpg: 'codicon-file-media',
  gif: 'codicon-file-media',
  ico: 'codicon-file-media',
  wasm: 'codicon-file-binary',
};

function iconForFile(name) {
  const ext = name.includes('.') ? name.split('.').pop().toLowerCase() : '';
  return EXT_ICON_MAP[ext] || 'codicon-file';
}

export class Sidebar {
  constructor(app) {
    this.app = app;
    this.el = document.getElementById('sidebar');
    this.contentEl = document.getElementById('sidebar-content');
    this.workspacePath = null;
    this.expandedDirs = new Set();
    this.currentView = 'explorer';
  }

  render() {
    if (!this.contentEl) {
      this.contentEl = this.el.querySelector('.sidebar-content') || this.el;
    }
    this.switchView('explorer');
  }

  switchView(viewId) {
    this.currentView = viewId;
    const header = this.el.querySelector('.sidebar-title');
    if (header) header.textContent = viewId.toUpperCase();

    if (viewId === 'explorer') {
      if (this.workspacePath) {
        this.loadTree(this.workspacePath);
      } else {
        this.showOpenFolder();
      }
    } else {
      this.contentEl.innerHTML = `<div class="sidebar-placeholder" style="padding:12px;opacity:0.5;">${viewId.charAt(0).toUpperCase() + viewId.slice(1)} — coming soon</div>`;
    }
  }

  showOpenFolder() {
    this.contentEl.innerHTML = '';
    const wrapper = document.createElement('div');
    wrapper.style.cssText = 'display:flex;flex-direction:column;align-items:center;padding:20px 12px;gap:12px;';

    const msg = document.createElement('p');
    msg.style.cssText = 'opacity:0.7;font-size:12px;margin:0;';
    msg.textContent = 'You have not yet opened a folder.';
    wrapper.appendChild(msg);

    const btn = document.createElement('button');
    btn.className = 'sidebar-open-folder-btn';
    btn.style.cssText = 'display:inline-flex;align-items:center;gap:6px;padding:6px 14px;background:var(--vscode-button-background, #0078d4);color:var(--vscode-button-foreground, #fff);border:none;border-radius:3px;font-size:12px;cursor:pointer;';
    btn.innerHTML = '<span class="codicon codicon-folder-opened"></span> Open Folder';
    btn.addEventListener('click', () => this.promptOpenFolder());
    wrapper.appendChild(btn);

    this.contentEl.appendChild(wrapper);
  }

  async promptOpenFolder() {
    try {
      const { open } = window.__TAURI__.dialog;
      const selected = await open({ directory: true });
      if (selected) this.openWorkspace(selected);
    } catch {
      const path = prompt('Enter folder path:');
      if (path) this.openWorkspace(path);
    }
  }

  async openWorkspace(path) {
    this.workspacePath = path;
    this.expandedDirs.clear();
    this.expandedDirs.add(path);
    await this.loadTree(path);
  }

  async loadTree(rootPath) {
    this.contentEl.innerHTML = '';
    try {
      const entries = await this.app.api.readDir(rootPath);
      const sorted = this.sortEntries(entries);
      const tree = this.buildTree(sorted, rootPath, 0);
      this.contentEl.appendChild(tree);
    } catch (err) {
      this.contentEl.innerHTML = `<div style="padding:8px 12px;color:var(--vscode-errorForeground,#f44);">Error: ${err.message || err}</div>`;
    }
  }

  sortEntries(entries) {
    const dirs = entries.filter(e => e.is_dir).sort((a, b) => a.name.localeCompare(b.name));
    const files = entries.filter(e => !e.is_dir).sort((a, b) => a.name.localeCompare(b.name));
    return [...dirs, ...files];
  }

  buildTree(entries, parentPath, depth) {
    const container = document.createElement('div');
    container.className = 'tree-group';

    for (const entry of entries) {
      const fullPath = parentPath.replace(/\/$/, '') + '/' + entry.name;
      const row = document.createElement('div');
      row.className = 'tree-row';
      row.style.paddingLeft = `${8 + depth * 20}px`;

      if (entry.is_dir) {
        const expanded = this.expandedDirs.has(fullPath);
        const chevron = document.createElement('span');
        chevron.className = `codicon ${expanded ? 'codicon-chevron-down' : 'codicon-chevron-right'}`;
        chevron.style.cssText = 'font-size:12px;margin-right:2px;min-width:16px;text-align:center;';
        row.appendChild(chevron);

        const icon = document.createElement('span');
        icon.className = `codicon ${expanded ? 'codicon-folder-opened' : 'codicon-folder'}`;
        icon.style.cssText = 'margin-right:4px;';
        row.appendChild(icon);

        const label = document.createElement('span');
        label.className = 'tree-label';
        label.textContent = entry.name;
        row.appendChild(label);

        container.appendChild(row);

        const childContainer = document.createElement('div');
        childContainer.className = 'tree-children';
        childContainer.style.display = expanded ? '' : 'none';
        container.appendChild(childContainer);

        if (expanded) this.loadChildren(fullPath, childContainer, depth + 1);

        row.addEventListener('click', () => {
          this.toggleDir(fullPath, childContainer, chevron, icon, depth + 1);
        });
      } else {
        const spacer = document.createElement('span');
        spacer.style.cssText = 'min-width:16px;display:inline-block;';
        row.appendChild(spacer);

        const icon = document.createElement('span');
        icon.className = `codicon ${iconForFile(entry.name)}`;
        icon.style.cssText = 'margin-right:4px;';
        row.appendChild(icon);

        const label = document.createElement('span');
        label.className = 'tree-label';
        label.textContent = entry.name;
        row.appendChild(label);

        container.appendChild(row);

        row.addEventListener('click', () => {
          this.contentEl.querySelectorAll('.tree-row.selected').forEach(r => r.classList.remove('selected'));
          row.classList.add('selected');
          this.app.openFile(fullPath);
        });
      }
    }
    return container;
  }

  async toggleDir(path, childContainer, chevronEl, iconEl, depth) {
    if (this.expandedDirs.has(path)) {
      this.expandedDirs.delete(path);
      childContainer.style.display = 'none';
      chevronEl.className = 'codicon codicon-chevron-right';
      iconEl.className = 'codicon codicon-folder';
    } else {
      this.expandedDirs.add(path);
      childContainer.style.display = '';
      chevronEl.className = 'codicon codicon-chevron-down';
      iconEl.className = 'codicon codicon-folder-opened';
      await this.loadChildren(path, childContainer, depth);
    }
  }

  async loadChildren(dirPath, container, depth) {
    container.innerHTML = '';
    try {
      const entries = await this.app.api.readDir(dirPath);
      const sorted = this.sortEntries(entries);
      const tree = this.buildTree(sorted, dirPath, depth);
      container.appendChild(tree);
    } catch {
      container.innerHTML = '<span style="padding:4px 8px;opacity:0.5;font-size:11px;">Failed to load</span>';
    }
  }
}
