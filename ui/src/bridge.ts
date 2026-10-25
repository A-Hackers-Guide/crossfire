// One API for the UI, three possible backends:
//   • Tauri   — the primary shell; the Rust engine is linked in-process.
//   • Electron — the fallback shell; it spawns the engine CLI as a sidecar.
//   • Demo     — plain browser (dev/preview); a scripted mock engine.
import type {
  BuildRequest, BuildResult, ConvertRequest, ConvertResult, Os, ProgressEvent, Question, ScanReport, Toolchain,
} from './types';
import { mockEngine } from './mock';

export type BackendKind = 'tauri' | 'electron' | 'demo';

export interface Engine {
  kind: BackendKind;
  hostOs: () => Promise<Os>;
  pickFolder: (title: string) => Promise<string | null>;
  pickFile: (title: string, extensions?: string[]) => Promise<string | null>;
  openPath: (path: string) => Promise<void>;
  scan: (path: string) => Promise<ScanReport>;
  questions: (scan: ScanReport, targets: Os[]) => Promise<Question[]>;
  convert: (req: ConvertRequest) => Promise<ConvertResult>;
  toolchains: () => Promise<Toolchain[]>;
  build: (req: BuildRequest) => Promise<BuildResult>;
  testWithGptk: (exe: string) => Promise<string>;
  onProgress: (cb: (e: ProgressEvent) => void) => () => void;
}

/** Shape exposed by apps/electron/preload.cjs via contextBridge. */
interface ElectronBridge {
  invoke: (cmd: string, args?: unknown) => Promise<unknown>;
  onProgress: (cb: (e: ProgressEvent) => void) => () => void;
}

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
    crossfire?: ElectronBridge;
  }
}

async function tauriEngine(): Promise<Engine> {
  const { invoke } = await import('@tauri-apps/api/core');
  const { listen } = await import('@tauri-apps/api/event');
  const dialog = await import('@tauri-apps/plugin-dialog');

  return {
    kind: 'tauri',
    hostOs: () => invoke<Os>('host_os'),
    pickFolder: async (title) => (await dialog.open({ directory: true, title })) as string | null,
    pickFile: async (title, extensions) =>
      (await dialog.open({ title, filters: extensions ? [{ name: 'Files', extensions }] : undefined })) as string | null,
    openPath: (path) => invoke('open_path', { path }),
    scan: (path) => invoke<ScanReport>('scan', { path }),
    questions: (scan, targets) => invoke<Question[]>('questions', { scan, targets }),
    convert: (req) => invoke<ConvertResult>('convert', { req }),
    toolchains: () => invoke<Toolchain[]>('toolchains'),
    build: (req) => invoke<BuildResult>('build', { req }),
    testWithGptk: (exe) => invoke<string>('test_with_gptk', { exe }),
    onProgress: (cb) => {
      const un = listen<ProgressEvent>('crossfire://progress', (e) => cb(e.payload));
      return () => { un.then((f) => f()); };
    },
  };
}

function electronEngine(b: ElectronBridge): Engine {
  const call = <T,>(cmd: string, args?: unknown) => b.invoke(cmd, args) as Promise<T>;
  return {
    kind: 'electron',
    hostOs: () => call<Os>('hostOs'),
    pickFolder: (title) => call<string | null>('pickFolder', { title }),
    pickFile: (title, extensions) => call<string | null>('pickFile', { title, extensions }),
    openPath: (path) => call('openPath', { path }),
    scan: (path) => call<ScanReport>('scan', { path }),
    questions: (scan, targets) => call<Question[]>('questions', { scan, targets }),
    convert: (req) => call<ConvertResult>('convert', { req }),
    toolchains: () => call<Toolchain[]>('toolchains'),
    build: (req) => call<BuildResult>('build', { req }),
    testWithGptk: (exe) => call<string>('gptk', { exe }),
    onProgress: b.onProgress,
  };
}

let cached: Promise<Engine> | null = null;

export function getEngine(): Promise<Engine> {
  if (!cached) {
    if (window.__TAURI_INTERNALS__) cached = tauriEngine();
    else if (window.crossfire) cached = Promise.resolve(electronEngine(window.crossfire));
    else cached = Promise.resolve(mockEngine());
  }
  return cached;
}
