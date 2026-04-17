let _invoke = null;

function getInvoke() {
  if (_invoke) return _invoke;
  if (window.__TAURI__?.core?.invoke) {
    _invoke = window.__TAURI__.core.invoke;
    return _invoke;
  }
  return null;
}

export async function invoke(cmd, args = {}) {
  const fn = getInvoke();
  if (!fn) {
    console.warn(`[SideX] invoke(${cmd}) — Tauri not available`);
    return null;
  }
  try {
    return await fn(cmd, args);
  } catch (e) {
    console.error(`[SideX] ${cmd} failed:`, e);
    throw e;
  }
}

export class TauriAPI {
  constructor() { this.ready = false; }
  async init() {
    await window.__SIDEX_READY__;
    this.ready = !!getInvoke();
    console.log(`[SideX] Tauri API: ${this.ready ? 'connected' : 'browser mode'}`);
  }
  async readFile(path) { return invoke('read_file', { path }); }
  async writeFile(path, content) { return invoke('write_file', { path, content }); }
  async readDir(path) { return invoke('read_dir', { path }); }
  async stat(path) { return invoke('stat', { path }); }
  async exists(path) { return invoke('exists', { path }); }
  async gitStatus(repoRoot) { return invoke('git_status', { repoRoot }); }
  async gitBranches(repoRoot) { return invoke('git_branches', { repoRoot }); }
  async searchFiles(dir, pattern) { return invoke('search_files', { dir, pattern }); }
  async detectLanguage(filename) { return invoke('syntax_detect_language', { filename }); }
  async getLanguages() { return invoke('syntax_get_languages'); }
  async termSpawn(opts) { return invoke('term_spawn', opts); }
  async termWrite(id, data) { return invoke('term_write', { id, data }); }
  async termRead(id) { return invoke('term_read', { id }); }
  async getDefaultShell() { return invoke('get_default_shell'); }
}
