import { TauriAPI } from './tauri-bridge.js';
import { Workbench } from './workbench.js';
import { EditorComponent } from './editor.js';
import { Sidebar } from './sidebar.js';
import { StatusBar } from './statusbar.js';
import { ActivityBar } from './activitybar.js';
import { TitleBar } from './titlebar.js';
import { TabBar } from './tabs.js';
import { Panel } from './panel.js';

class SideXApp {
  constructor() {
    this.api = new TauriAPI();
    this.workbench = new Workbench(this);
    this.editor = new EditorComponent(this);
    this.sidebar = new Sidebar(this);
    this.statusbar = new StatusBar(this);
    this.activitybar = new ActivityBar(this);
    this.titlebar = new TitleBar(this);
    this.tabbar = new TabBar(this);
    this.panel = new Panel(this);

    this.state = {
      openFiles: [],
      activeFile: null,
      sidebarVisible: true,
      panelVisible: true,
      theme: 'dark',
    };
  }

  async init() {
    await this.api.init();

    this.titlebar.render();
    this.activitybar.render();
    this.sidebar.render();
    this.tabbar.render();
    this.panel.render();
    this.statusbar.render();
    this.workbench.render();

    this.bindWelcomeActions();
    this.setupKeyboardShortcuts();

    console.log('[SideX] App initialized');
  }

  openFile(path) {
    this.tabbar.openTab(path);
  }

  showWelcome() {
    const welcome = document.getElementById('editor-welcome');
    const content = document.getElementById('editor-content');
    if (welcome) welcome.style.display = '';
    if (content) content.style.display = 'none';
  }

  bindWelcomeActions() {
    const btnNewFile = document.getElementById('btn-new-file');
    const btnOpenFile = document.getElementById('btn-open-file');
    const btnOpenFolder = document.getElementById('btn-open-folder');

    if (btnNewFile) {
      btnNewFile.addEventListener('click', () => {
        this.editor.newFile();
      });
    }

    if (btnOpenFile) {
      btnOpenFile.addEventListener('click', async () => {
        try {
          const { open } = window.__TAURI__.dialog;
          const selected = await open({ multiple: false });
          if (selected) this.openFile(selected);
        } catch {
          console.warn('[SideX] File dialog not available');
        }
      });
    }

    if (btnOpenFolder) {
      btnOpenFolder.addEventListener('click', () => {
        this.sidebar.promptOpenFolder();
      });
    }
  }

  setupKeyboardShortcuts() {
    document.addEventListener('keydown', (e) => {
      const meta = e.metaKey || e.ctrlKey;

      if (meta && e.key === 's') {
        e.preventDefault();
        this.editor.save();
      }
      if (meta && e.key === 'b') {
        e.preventDefault();
        this.workbench.toggleSidebar();
      }
      if (meta && e.key === 'j') {
        e.preventDefault();
        this.workbench.togglePanel();
      }
      if (meta && e.key === 'p') {
        e.preventDefault();
      }
    });
  }
}

const app = new SideXApp();
document.addEventListener('DOMContentLoaded', async () => {
  if (window.__SIDEX_READY__) await window.__SIDEX_READY__;
  app.init();
});
