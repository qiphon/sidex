export class ActivityBar {
  constructor(app) {
    this.app = app;
    this.container = document.getElementById('activitybar');
    this.activeView = 'explorer';
    this.items = [
      { id: 'explorer', icon: 'codicon-files', tooltip: 'Explorer (⇧⌘E)' },
      { id: 'search', icon: 'codicon-search', tooltip: 'Search (⇧⌘F)' },
      { id: 'scm', icon: 'codicon-source-control', tooltip: 'Source Control (⌃⇧G)' },
      { id: 'debug', icon: 'codicon-debug-alt', tooltip: 'Run and Debug (⇧⌘D)' },
      { id: 'extensions', icon: 'codicon-extensions', tooltip: 'Extensions (⇧⌘X)' },
    ];
  }

  render() {
    this.container.innerHTML = '';
    const topGroup = document.createElement('div');
    topGroup.className = 'activity-top';

    for (const item of this.items) {
      const el = document.createElement('div');
      el.className = `activity-item ${item.id === this.activeView ? 'active' : ''}`;
      el.title = item.tooltip;
      el.innerHTML = `<div class="codicon ${item.icon}"></div>`;
      el.addEventListener('click', () => this.setActive(item.id));
      topGroup.appendChild(el);
    }

    const bottomGroup = document.createElement('div');
    bottomGroup.className = 'activity-bottom';

    const settingsBtn = document.createElement('div');
    settingsBtn.className = 'activity-item';
    settingsBtn.title = 'Manage';
    settingsBtn.innerHTML = '<div class="codicon codicon-settings-gear"></div>';
    bottomGroup.appendChild(settingsBtn);

    this.container.appendChild(topGroup);
    this.container.appendChild(bottomGroup);
  }

  setActive(id) {
    this.activeView = id;
    this.render();
    this.app.sidebar.switchView(id);
  }
}
