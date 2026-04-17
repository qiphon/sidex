export class EditorComponent {
  constructor(app) {
    this.app = app;
    this.container = document.getElementById('editor-container');
    this.lines = [];
    this.cursor = { line: 0, col: 0 };
    this.selection = null;
    this.scrollTop = 0;
    this.scrollLeft = 0;
    this.lineHeight = 20;
    this.charWidth = 8.4;
    this.fontSize = 14;
    this.fontFamily = "'JetBrains Mono', 'Fira Code', 'Cascadia Code', 'Consolas', monospace";
    this.gutterWidth = 60;
    this.visibleLines = 0;
    this.isDirty = false;
    this.language = 'plaintext';
    this.filePath = null;
  }

  render() {
    this.container.innerHTML = '';

    this.gutterEl = document.createElement('div');
    this.gutterEl.className = 'editor-gutter';

    this.contentEl = document.createElement('div');
    this.contentEl.className = 'editor-content';
    this.contentEl.contentEditable = false;

    this.cursorEl = document.createElement('div');
    this.cursorEl.className = 'editor-cursor';

    this.selectionEl = document.createElement('div');
    this.selectionEl.className = 'editor-selections';

    this.inputEl = document.createElement('textarea');
    this.inputEl.className = 'editor-input';
    this.inputEl.autocomplete = 'off';
    this.inputEl.autocapitalize = 'off';
    this.inputEl.spellcheck = false;

    this.container.appendChild(this.gutterEl);
    this.container.appendChild(this.selectionEl);
    this.container.appendChild(this.contentEl);
    this.container.appendChild(this.cursorEl);
    this.container.appendChild(this.inputEl);

    this.setupEvents();
    this.calculateMetrics();
    this.renderContent();
  }

  calculateMetrics() {
    const rect = this.container.getBoundingClientRect();
    this.visibleLines = Math.ceil(rect.height / this.lineHeight);
    const canvas = document.createElement('canvas');
    const ctx = canvas.getContext('2d');
    ctx.font = `${this.fontSize}px ${this.fontFamily}`;
    this.charWidth = ctx.measureText('M').width;
  }

  async openFile(path) {
    const content = await this.app.api.readFile(path);
    this.filePath = path;
    this.lines = content.split('\n');
    this.cursor = { line: 0, col: 0 };
    this.scrollTop = 0;
    this.isDirty = false;

    const filename = path.split('/').pop();
    this.language = await this.app.api.detectLanguage(filename).catch(() => 'plaintext');

    this.renderContent();
    this.updateCursor();
    this.app.statusbar.update({ line: 1, col: 1, language: this.language, encoding: 'UTF-8', eol: 'LF' });
  }

  renderContent() {
    const startLine = Math.floor(this.scrollTop / this.lineHeight);
    const endLine = Math.min(startLine + this.visibleLines + 5, this.lines.length);

    let gutterHtml = '';
    for (let i = startLine; i < endLine; i++) {
      const isActive = i === this.cursor.line;
      gutterHtml += `<div class="gutter-line ${isActive ? 'active' : ''}" style="height:${this.lineHeight}px">${i + 1}</div>`;
    }
    this.gutterEl.innerHTML = gutterHtml;

    let contentHtml = '';
    for (let i = startLine; i < endLine; i++) {
      const line = this.lines[i] || '';
      const isActive = i === this.cursor.line;
      const escapedLine = this.escapeHtml(line) || '&nbsp;';
      contentHtml += `<div class="editor-line ${isActive ? 'current-line' : ''}" data-line="${i}" style="height:${this.lineHeight}px;padding-left:${this.gutterWidth}px">${escapedLine}</div>`;
    }
    this.contentEl.innerHTML = contentHtml;
    this.contentEl.style.transform = `translateY(-${this.scrollTop % this.lineHeight}px)`;
  }

  updateCursor() {
    const x = this.gutterWidth + (this.cursor.col * this.charWidth) - this.scrollLeft;
    const y = (this.cursor.line * this.lineHeight) - this.scrollTop;
    this.cursorEl.style.left = `${x}px`;
    this.cursorEl.style.top = `${y}px`;
    this.cursorEl.style.height = `${this.lineHeight}px`;

    this.cursorEl.classList.remove('blink');
    void this.cursorEl.offsetWidth;
    this.cursorEl.classList.add('blink');
  }

  setupEvents() {
    this.container.addEventListener('mousedown', (e) => {
      this.inputEl.focus();
      const rect = this.container.getBoundingClientRect();
      const x = e.clientX - rect.left - this.gutterWidth + this.scrollLeft;
      const y = e.clientY - rect.top + this.scrollTop;
      const line = Math.min(Math.floor(y / this.lineHeight), this.lines.length - 1);
      const col = Math.min(Math.round(x / this.charWidth), (this.lines[line] || '').length);
      this.cursor = { line: Math.max(0, line), col: Math.max(0, col) };
      this.updateCursor();
      this.renderContent();
      this.app.statusbar.update({ line: this.cursor.line + 1, col: this.cursor.col + 1 });
    });

    this.inputEl.addEventListener('input', (e) => {
      const text = e.data;
      if (text) this.insertText(text);
    });

    this.inputEl.addEventListener('keydown', (e) => this.handleKeydown(e));

    this.container.addEventListener('wheel', (e) => {
      e.preventDefault();
      this.scrollTop = Math.max(0, this.scrollTop + e.deltaY);
      const maxScroll = Math.max(0, (this.lines.length * this.lineHeight) - this.container.clientHeight);
      this.scrollTop = Math.min(this.scrollTop, maxScroll);
      this.renderContent();
      this.updateCursor();
    });
  }

  handleKeydown(e) {
    switch (e.key) {
      case 'ArrowUp': e.preventDefault(); this.moveCursor(0, -1); break;
      case 'ArrowDown': e.preventDefault(); this.moveCursor(0, 1); break;
      case 'ArrowLeft': e.preventDefault(); this.moveCursor(-1, 0); break;
      case 'ArrowRight': e.preventDefault(); this.moveCursor(1, 0); break;
      case 'Home': e.preventDefault(); this.cursor.col = 0; this.updateCursor(); break;
      case 'End': e.preventDefault(); this.cursor.col = (this.lines[this.cursor.line] || '').length; this.updateCursor(); break;
      case 'Enter': e.preventDefault(); this.insertNewline(); break;
      case 'Backspace': e.preventDefault(); this.deleteLeft(); break;
      case 'Delete': e.preventDefault(); this.deleteRight(); break;
      case 'Tab': e.preventDefault(); this.insertText('  '); break;
    }
    this.app.statusbar.update({ line: this.cursor.line + 1, col: this.cursor.col + 1 });
  }

  insertText(text) {
    const line = this.lines[this.cursor.line] || '';
    this.lines[this.cursor.line] = line.slice(0, this.cursor.col) + text + line.slice(this.cursor.col);
    this.cursor.col += text.length;
    this.isDirty = true;
    this.renderContent();
    this.updateCursor();
  }

  insertNewline() {
    const line = this.lines[this.cursor.line] || '';
    const before = line.slice(0, this.cursor.col);
    const after = line.slice(this.cursor.col);
    const indent = before.match(/^(\s*)/)[1];
    this.lines[this.cursor.line] = before;
    this.lines.splice(this.cursor.line + 1, 0, indent + after);
    this.cursor.line++;
    this.cursor.col = indent.length;
    this.isDirty = true;
    this.renderContent();
    this.updateCursor();
  }

  deleteLeft() {
    if (this.cursor.col > 0) {
      const line = this.lines[this.cursor.line];
      this.lines[this.cursor.line] = line.slice(0, this.cursor.col - 1) + line.slice(this.cursor.col);
      this.cursor.col--;
    } else if (this.cursor.line > 0) {
      const prevLine = this.lines[this.cursor.line - 1];
      const curLine = this.lines[this.cursor.line];
      this.cursor.col = prevLine.length;
      this.lines[this.cursor.line - 1] = prevLine + curLine;
      this.lines.splice(this.cursor.line, 1);
      this.cursor.line--;
    }
    this.isDirty = true;
    this.renderContent();
    this.updateCursor();
  }

  deleteRight() {
    const line = this.lines[this.cursor.line] || '';
    if (this.cursor.col < line.length) {
      this.lines[this.cursor.line] = line.slice(0, this.cursor.col) + line.slice(this.cursor.col + 1);
    } else if (this.cursor.line < this.lines.length - 1) {
      this.lines[this.cursor.line] = line + this.lines[this.cursor.line + 1];
      this.lines.splice(this.cursor.line + 1, 1);
    }
    this.isDirty = true;
    this.renderContent();
    this.updateCursor();
  }

  moveCursor(dx, dy) {
    if (dy !== 0) {
      this.cursor.line = Math.max(0, Math.min(this.lines.length - 1, this.cursor.line + dy));
      this.cursor.col = Math.min(this.cursor.col, (this.lines[this.cursor.line] || '').length);
    }
    if (dx !== 0) {
      this.cursor.col = Math.max(0, Math.min((this.lines[this.cursor.line] || '').length, this.cursor.col + dx));
    }
    this.ensureCursorVisible();
    this.updateCursor();
    this.renderContent();
  }

  ensureCursorVisible() {
    const cursorY = this.cursor.line * this.lineHeight;
    const viewTop = this.scrollTop;
    const viewBottom = this.scrollTop + this.container.clientHeight - this.lineHeight;
    if (cursorY < viewTop) this.scrollTop = cursorY;
    if (cursorY > viewBottom) this.scrollTop = cursorY - this.container.clientHeight + this.lineHeight * 2;
  }

  async save() {
    if (!this.filePath || !this.isDirty) return;
    const content = this.lines.join('\n');
    await this.app.api.writeFile(this.filePath, content);
    this.isDirty = false;
    this.app.tabbar.updateDirtyState(this.filePath, false);
  }

  getContent() { return this.lines.join('\n'); }

  escapeHtml(text) {
    return text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/ /g, '&nbsp;');
  }
}
