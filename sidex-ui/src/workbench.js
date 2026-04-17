export class Workbench {
  constructor(app) {
    this.app = app;
    this.el = document.getElementById('workbench');
  }

  render() {
    this.el.classList.add('workbench-ready');
    this.applyLayout();
    this.setupSashes();
  }

  applyLayout() {
    const sidebar = document.getElementById('sidebar');
    const sashSidebar = document.getElementById('sash-sidebar');
    const panel = document.getElementById('panel');
    const sashPanel = document.getElementById('sash-panel');

    if (sidebar) sidebar.style.display = this.app.state.sidebarVisible ? '' : 'none';
    if (sashSidebar) sashSidebar.style.display = this.app.state.sidebarVisible ? '' : 'none';
    if (panel) panel.style.display = this.app.state.panelVisible ? '' : 'none';
    if (sashPanel) sashPanel.style.display = this.app.state.panelVisible ? '' : 'none';
  }

  setupSashes() {
    const sashSidebar = document.getElementById('sash-sidebar');
    if (sashSidebar) {
      sashSidebar.addEventListener('mousedown', (e) => this.startResize(e, 'sidebar'));
    }
    const sashPanel = document.getElementById('sash-panel');
    if (sashPanel) {
      sashPanel.addEventListener('mousedown', (e) => this.startResize(e, 'panel'));
    }
  }

  startResize(e, target) {
    e.preventDefault();
    const el = document.getElementById(target === 'sidebar' ? 'sidebar' : 'panel');
    if (!el) return;
    const startPos = target === 'sidebar' ? e.clientX : e.clientY;
    const startSize = target === 'sidebar' ? el.offsetWidth : el.offsetHeight;

    const onMove = (ev) => {
      const delta = target === 'sidebar'
        ? ev.clientX - startPos
        : startPos - ev.clientY;
      const newSize = Math.max(150, startSize + delta);
      if (target === 'sidebar') {
        el.style.width = `${newSize}px`;
      } else {
        el.style.height = `${newSize}px`;
      }
    };

    const onUp = () => {
      document.removeEventListener('mousemove', onMove);
      document.removeEventListener('mouseup', onUp);
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
    };

    document.body.style.cursor = target === 'sidebar' ? 'col-resize' : 'row-resize';
    document.body.style.userSelect = 'none';
    document.addEventListener('mousemove', onMove);
    document.addEventListener('mouseup', onUp);
  }

  toggleSidebar() {
    this.app.state.sidebarVisible = !this.app.state.sidebarVisible;
    this.applyLayout();
  }

  togglePanel() {
    this.app.state.panelVisible = !this.app.state.panelVisible;
    this.applyLayout();
  }

  openFile(path) {
    this.app.tabbar.openTab(path);
  }

  getActiveFile() {
    return this.app.state.activeFile;
  }

  setTheme(theme) {
    this.app.state.theme = theme;
    this.el.dataset.theme = theme;
  }
}
