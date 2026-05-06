import fs from 'fs';
import path from 'path';

function exists(p) {
  try {
    return fs.existsSync(p);
  } catch {
    return false;
  }
}

function isDir(p) {
  try {
    return fs.statSync(p).isDirectory();
  } catch {
    return false;
  }
}

function requirePath(label, p, kind) {
  if (kind === 'dir') {
    if (!isDir(p)) {
      throw new Error(`${label} missing directory: ${p}`);
    }
    return;
  }
  if (!exists(p)) {
    throw new Error(`${label} missing file: ${p}`);
  }
}

function main() {
  const root = process.cwd();

  const distDir = path.join(root, 'dist');
  if (!isDir(distDir)) {
    process.stdout.write('[verify] dist/ not found, skipping build resource checks\n');
    return;
  }

  const required = [
    { label: 'Built-in extensions', path: path.join(distDir, 'extensions'), kind: 'dir' },
    { label: 'Built-in extensions meta', path: path.join(distDir, 'extensions-meta.json'), kind: 'file' },
    { label: 'Extension host resources', path: path.join(root, 'src-tauri', 'extension-host'), kind: 'dir' },
    { label: 'Extension host server', path: path.join(root, 'src-tauri', 'extension-host', 'server.cjs'), kind: 'file' },
    { label: 'Extension host runtime', path: path.join(root, 'src-tauri', 'extension-host', 'host.cjs'), kind: 'file' },
  ];

  const missing = [];
  for (const item of required) {
    try {
      requirePath(item.label, item.path, item.kind);
    } catch (e) {
      missing.push(e.message);
    }
  }

  if (missing.length > 0) {
    process.stderr.write('[verify] build resources check failed:\n');
    for (const msg of missing) {
      process.stderr.write(`- ${msg}\n`);
    }
    process.exit(1);
  }

  process.stdout.write('[verify] build resources check passed\n');
}

main();

