export class StatusBar {
  constructor(app) {
    this.app = app;
    this.leftContainer = null;
    this.rightContainer = null;
  }

  render() {
    this.leftContainer = document.getElementById('statusbar-left');
    this.rightContainer = document.getElementById('statusbar-right');

    this.leftContainer.innerHTML = `
      <div class="status-item remote-indicator"><span class="codicon codicon-remote"></span></div>
      <div class="status-item"><span class="codicon codicon-git-branch"></span> main</div>
      <div class="status-item"><span class="codicon codicon-sync"></span></div>
      <div class="status-item error-info"><span class="codicon codicon-error"></span> 0 <span class="codicon codicon-warning"></span> 0</div>
    `;

    this.rightContainer.innerHTML = `
      <div class="status-item">Ln 1, Col 1</div>
      <div class="status-item">Spaces: 2</div>
      <div class="status-item">UTF-8</div>
      <div class="status-item">LF</div>
      <div class="status-item">Plain Text</div>
      <div class="status-item"><span class="codicon codicon-bell"></span></div>
    `;
  }

  update(info) {
    if (!this.rightContainer) return;
    const items = this.rightContainer.querySelectorAll('.status-item');
    if (info.line !== undefined && items[0]) items[0].textContent = `Ln ${info.line}, Col ${info.col}`;
    if (info.language && items[4]) items[4].textContent = info.language;
  }
}
