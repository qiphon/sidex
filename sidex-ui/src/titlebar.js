export class TitleBar {
  constructor(app) {
    this.app = app;
    this.el = document.getElementById('titlebar');
    this.titleTextEl = null;
    this.fileName = null;
    this.workspaceName = null;
  }

  render() {
    const dragRegion = this.el.querySelector('.titlebar-drag-region');
    if (dragRegion) dragRegion.setAttribute('data-tauri-drag-region', '');

    this.titleTextEl = this.el.querySelector('.titlebar-title');
    this.updateTitle();

    const minimizeBtn = this.el.querySelector('.window-control.minimize');
    const maximizeBtn = this.el.querySelector('.window-control.maximize');
    const closeBtn = this.el.querySelector('.window-control.close');

    if (minimizeBtn) minimizeBtn.addEventListener('click', () => this.windowAction('minimize'));
    if (maximizeBtn) maximizeBtn.addEventListener('click', () => this.windowAction('maximize'));
    if (closeBtn) closeBtn.addEventListener('click', () => this.windowAction('close'));
  }

  updateTitle() {
    const parts = [];
    if (this.fileName) parts.push(this.fileName);
    if (this.workspaceName) parts.push(this.workspaceName);
    parts.push('SideX');
    const title = parts.join(' — ');
    if (this.titleTextEl) this.titleTextEl.textContent = title;
    document.title = title;
  }

  setFileName(name) {
    this.fileName = name;
    this.updateTitle();
  }

  setWorkspace(name) {
    this.workspaceName = name;
    this.updateTitle();
  }

  async windowAction(action) {
    try {
      const win = window.__TAURI__?.window?.getCurrentWindow();
      if (!win) return;
      if (action === 'minimize') await win.minimize();
      else if (action === 'maximize') await win.toggleMaximize();
      else if (action === 'close') await win.close();
    } catch { /* no-op outside Tauri */ }
  }
}
