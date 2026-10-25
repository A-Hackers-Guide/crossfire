// Electron fallback shell for Crossfire. The UI is the same React build as the
// Tauri app; engine calls go to the `crossfire-engine` CLI bundled as a sidecar.
const { app, BrowserWindow, dialog, ipcMain, shell } = require('electron');
const { spawn } = require('node:child_process');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const exe = process.platform === 'win32' ? 'crossfire-engine.exe' : 'crossfire-engine';
const enginePath = app.isPackaged
  ? path.join(process.resourcesPath, 'bin', exe)
  : path.join(__dirname, '..', '..', 'engine', 'target', 'release', exe);

let win;

/** Run the engine CLI: JSON result on stdout, one JSON progress event per stderr line. */
function engine(args) {
  return new Promise((resolve, reject) => {
    const child = spawn(enginePath, args, { windowsHide: true });
    let out = '';
    let err = '';
    let partial = '';
    child.stdout.on('data', (d) => { out += d; });
    child.stderr.on('data', (d) => {
      partial += d;
      const lines = partial.split('\n');
      partial = lines.pop();
      for (const line of lines) {
        try { win?.webContents.send('crossfire:progress', JSON.parse(line)); } catch { err += line + '\n'; }
      }
    });
    child.on('error', reject);
    child.on('close', (code) => {
      if (code !== 0) return reject(new Error(err.trim() || `engine exited with code ${code}`));
      try { resolve(JSON.parse(out)); } catch (e) { reject(new Error(`bad engine output: ${e.message}`)); }
    });
  });
}

/** Write a request object to a temp file so large payloads never hit argv limits. */
function tempJson(obj) {
  const p = path.join(os.tmpdir(), `crossfire-${process.pid}-${Date.now()}.json`);
  fs.writeFileSync(p, JSON.stringify(obj));
  return p;
}

const hostOs = () => ({ win32: 'windows', darwin: 'macos', freebsd: 'freebsd' })[process.platform] ?? 'linux';

const handlers = {
  hostOs: async () => hostOs(),
  pickFolder: async ({ title }) => {
    const r = await dialog.showOpenDialog(win, { title, properties: ['openDirectory', 'createDirectory'] });
    return r.canceled ? null : r.filePaths[0];
  },
  pickFile: async ({ title, extensions }) => {
    const r = await dialog.showOpenDialog(win, { title, properties: ['openFile'], filters: extensions ? [{ name: 'Files', extensions }] : [] });
    return r.canceled ? null : r.filePaths[0];
  },
  openPath: async ({ path: p }) => { await shell.openPath(p); },
  scan: ({ path: p }) => engine(['scan', p]),
  questions: ({ scan, targets }) => engine(['questions', tempJson(scan), '--targets', targets.join(',')]),
  convert: ({ req }) => engine(['convert', tempJson(req)]),
  toolchains: () => engine(['toolchains']),
  build: ({ req }) => engine(['build', tempJson(req)]),
  gptk: ({ exe: e }) => engine(['gptk', e]),
};

ipcMain.handle('crossfire:invoke', async (_e, cmd, args) => {
  const h = handlers[cmd];
  if (!h) throw new Error(`unknown command: ${cmd}`);
  return h(args ?? {});
});

function createWindow() {
  win = new BrowserWindow({
    width: 1360, height: 880, minWidth: 980, minHeight: 640,
    backgroundColor: '#08070d', title: 'Crossfire',
    titleBarStyle: process.platform === 'darwin' ? 'hiddenInset' : 'default',
    webPreferences: { preload: path.join(__dirname, 'preload.cjs'), contextIsolation: true, sandbox: true },
  });
  const dist = app.isPackaged ? path.join(__dirname, 'ui', 'index.html') : path.join(__dirname, '..', '..', 'ui', 'dist', 'index.html');
  if (process.env.CROSSFIRE_DEV_URL) win.loadURL(process.env.CROSSFIRE_DEV_URL);
  else win.loadFile(dist);
}

app.whenReady().then(createWindow);
app.on('window-all-closed', () => { if (process.platform !== 'darwin') app.quit(); });
app.on('activate', () => { if (BrowserWindow.getAllWindows().length === 0) createWindow(); });
