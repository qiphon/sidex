const EXT_ICON_MAP = {
  js: 'codicon-file-code',
  ts: 'codicon-file-code',
  jsx: 'codicon-file-code',
  tsx: 'codicon-file-code',
  rs: 'codicon-file-code',
  py: 'codicon-file-code',
  go: 'codicon-file-code',
  json: 'codicon-json',
  md: 'codicon-markdown',
  html: 'codicon-file-code',
  css: 'codicon-file-code',
  toml: 'codicon-settings-gear',
  yaml: 'codicon-settings-gear',
  yml: 'codicon-settings-gear',
};

function iconForFile(name) {
  const ext = name.includes('.') ? name.split('.').pop().toLowerCase() : '';
  return EXT_ICON_MAP[ext] || 'codicon-file';
}

export class TabBar {
  constructor(app) {
    this.app = app;
    this.el = document.getElementById('tabs-container');
    this.tabs = [];
    this.activeIndex = -1;
  }

  render() {
    this.el.innerHTML = '';
    this.el.className = 'tab-bar';

    if (this.tabs.length === 0) return;

    for (let i = 0; i < this.tabs.length; i++) {
      const tab = this.tabs[i];
      const tabEl = document.createElement('div');
      tabEl.className = `tab${i === this.activeIndex ? ' active' : ''}${tab.dirty ? ' dirty' : ''}`;
      tabEl.dataset.index = i;

      const icon = document.createElement('span');
      icon.className = `codicon ${iconForFile(tab.name)}`;
      icon.style.cssText = 'margin-right:4px;font-size:14px;';
      tabEl.appendChild(icon);

      const label = document.createElement('span');
      label.className = 'tab-label';
      label.textContent = tab.name;
      label.title = tab.path;
      tabEl.appendChild(label);

      if (tab.dirty) {
        const dot = document.createElement('span');
        dot.className = 'tab-dirty-dot';
        dot.style.cssText = 'width:8px;height:8px;border-radius:50%;background:var(--vscode-editor-foreground, #ccc);margin-left:6px;display:inline-block;';
        tabEl.appendChild(dot);
      }

      const closeBtn = document.createElement('div');
      closeBtn.className = 'tab-close';
      closeBtn.title = 'Close';
      closeBtn.innerHTML = '<span class="codicon codicon-close"></span>';
      closeBtn.addEventListener('click', (e) => {
        e.stopPropagation();
        this.closeTab(i);
      });
      tabEl.appendChild(closeBtn);

      tabEl.addEventListener('click', () => this.activateTab(i));
      tabEl.addEventListener('auxclick', (e) => {
        if (e.button === 1) this.closeTab(i);
      });

      this.el.appendChild(tabEl);
    }
  }

  openTab(path) {
    const existing = this.tabs.findIndex(t => t.path === path);
    if (existing !== -1) {
      this.activateTab(existing);
      return;
    }

    const name = path.split('/').pop();
    this.tabs.push({ path, name, dirty: false });
    this.activeIndex = this.tabs.length - 1;
    this.render();
    this.notifySwitch();
  }

  activateTab(index) {
    if (index < 0 || index >= this.tabs.length) return;
    this.activeIndex = index;
    this.render();
    this.notifySwitch();
  }

  closeTab(index) {
    if (index < 0 || index >= this.tabs.length) return;
    const wasActive = index === this.activeIndex;
    this.tabs.splice(index, 1);

    if (this.tabs.length === 0) {
      this.activeIndex = -1;
    } else if (wasActive) {
      this.activeIndex = Math.min(index, this.tabs.length - 1);
    } else if (index < this.activeIndex) {
      this.activeIndex--;
    }

    this.render();
    if (wasActive) this.notifySwitch();
  }

  markDirty(path, dirty = true) {
    const tab = this.tabs.find(t => t.path === path);
    if (tab) {
      tab.dirty = dirty;
      this.render();
    }
  }

  notifySwitch() {
    const tab = this.tabs[this.activeIndex];
    if (tab) {
      this.app.titlebar.setFileName(tab.name);
      this.app.editor.openFile(tab.path);
    } else {
      this.app.titlebar.setFileName(null);
      this.app.showWelcome();
    }
  }

  getActiveTab() {
    return this.tabs[this.activeIndex] || null;
  }
}
