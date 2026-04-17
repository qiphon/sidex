export class Panel {
  constructor(app) {
    this.app = app;
    this.el = document.getElementById('panel');
    this.activeTab = 'terminal';
    this.content = { terminal: '', problems: '', output: '' };
    this.tabsRow = null;
    this.body = null;
  }

  render() {
    this.tabsRow = this.el.querySelector('.panel-tabs');
    this.body = this.el.querySelector('.panel-content');

    if (!this.tabsRow || !this.body) return;

    const tabs = this.tabsRow.querySelectorAll('.panel-tab');
    tabs.forEach(tab => {
      tab.addEventListener('click', () => {
        const panelId = tab.dataset.panel;
        if (panelId) this.switchTab(panelId);
      });
    });

    this.renderContent();
  }

  switchTab(id) {
    this.activeTab = id;
    const tabs = this.tabsRow.querySelectorAll('.panel-tab');
    tabs.forEach(tab => {
      tab.classList.toggle('active', tab.dataset.panel === id);
    });
    this.renderContent();
  }

  renderContent() {
    if (!this.body) return;
    this.body.innerHTML = '';

    if (this.activeTab === 'terminal') {
      const term = document.createElement('div');
      term.id = 'terminal-content';
      term.className = 'terminal-content';
      const pre = document.createElement('pre');
      pre.style.cssText = 'margin:0;padding:8px 12px;font-family:var(--font-mono);font-size:13px;color:var(--vscode-terminal-foreground, #ccc);white-space:pre-wrap;';
      pre.textContent = this.content.terminal || '$ ';
      term.appendChild(pre);
      this.body.appendChild(term);
    } else if (this.activeTab === 'problems') {
      const div = document.createElement('div');
      div.style.cssText = 'padding:8px 12px;font-size:12px;opacity:0.7;';
      div.innerHTML = this.content.problems || '<span class="codicon codicon-info" style="margin-right:4px;"></span> No problems have been detected in the workspace.';
      this.body.appendChild(div);
    } else if (this.activeTab === 'output') {
      const pre = document.createElement('pre');
      pre.style.cssText = 'margin:0;padding:8px 12px;font-family:var(--font-mono);font-size:13px;color:var(--vscode-terminal-foreground, #ccc);white-space:pre-wrap;';
      pre.textContent = this.content.output || '';
      this.body.appendChild(pre);
    }
  }

  appendTerminal(text) {
    this.content.terminal += text;
    if (this.activeTab === 'terminal') this.renderContent();
  }

  setProblems(text) {
    this.content.problems = text;
    if (this.activeTab === 'problems') this.renderContent();
  }

  appendOutput(text) {
    this.content.output += text;
    if (this.activeTab === 'output') this.renderContent();
  }

  clearTab(id) {
    this.content[id] = '';
    if (this.activeTab === id) this.renderContent();
  }
}
