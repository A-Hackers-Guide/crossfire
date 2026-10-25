// Exposes the same small API the UI's bridge.ts expects as `window.crossfire`.
const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('crossfire', {
  invoke: (cmd, args) => ipcRenderer.invoke('crossfire:invoke', cmd, args),
  onProgress: (cb) => {
    const listener = (_e, ev) => cb(ev);
    ipcRenderer.on('crossfire:progress', listener);
    return () => ipcRenderer.removeListener('crossfire:progress', listener);
  },
});
