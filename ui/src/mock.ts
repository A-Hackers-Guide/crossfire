// Demo backend: used when the UI runs in a plain browser (vite dev / preview).
// It plays back a believable port of a Win32 + DirectX 11 game so every screen can be explored.
import type { Engine } from './bridge';
import type {
  BuildResult, ConvertResult, Finding, Os, ProgressEvent, Question, ScanReport, Toolchain,
} from './types';

const listeners = new Set<(e: ProgressEvent) => void>();
const emit = (e: Omit<ProgressEvent, 'level'> & { level?: ProgressEvent['level'] }) =>
  listeners.forEach((l) => l({ level: 'info', ...e }));
const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));

const FILES = [
  'src/main.cpp', 'src/platform/win32_window.cpp', 'src/platform/win32_input.cpp', 'src/render/d3d11_device.cpp',
  'src/render/d3d11_pipeline.cpp', 'src/render/shaders/sprite.hlsl', 'src/audio/xaudio2_mixer.cpp', 'src/game/world.cpp',
  'src/game/player.cpp', 'src/game/physics.cpp', 'src/core/filesystem.cpp', 'src/core/timer.cpp', 'src/core/threads.cpp',
  'src/core/config_registry.cpp', 'src/ui/hud.cpp', 'tools/pack_assets.bat', 'EmberQuest.vcxproj',
];

const f = (
  id: number, file: string, line: number, api: string, category: Finding['category'], severity: Finding['severity'],
  snippet: string, message: string, suggestions: Finding['suggestions'],
): Finding => ({ id: `F${id}`, file, line, api, category, severity, os: ['windows'], snippet, message, suggestions });

const FINDINGS: Finding[] = [
  f(1, 'src/platform/win32_window.cpp', 41, 'CreateWindowExW', 'windowing', 'blocker',
    'HWND hwnd = CreateWindowExW(0, L"EmberQuest", title, WS_OVERLAPPEDWINDOW,\n    CW_USEDEFAULT, CW_USEDEFAULT, w, h, nullptr, nullptr, hInst, nullptr);',
    'Native Win32 window creation and message loop (WndProc).',
    { macos: 'SDL_CreateWindow (Cocoa backend) or NSWindow via Objective-C++', linux: 'SDL_CreateWindow (Wayland/X11)', freebsd: 'SDL_CreateWindow (X11/Wayland)' }),
  f(2, 'src/platform/win32_window.cpp', 88, 'WndProc / PeekMessage', 'windowing', 'high',
    'while (PeekMessageW(&msg, nullptr, 0, 0, PM_REMOVE)) {\n    TranslateMessage(&msg);\n    DispatchMessageW(&msg);\n}',
    'Win32 message pump drives the main loop.',
    { macos: 'SDL_PollEvent loop', linux: 'SDL_PollEvent loop', freebsd: 'SDL_PollEvent loop' }),
  f(3, 'src/render/d3d11_device.cpp', 23, 'D3D11CreateDeviceAndSwapChain', 'graphics', 'blocker',
    'hr = D3D11CreateDeviceAndSwapChain(nullptr, D3D_DRIVER_TYPE_HARDWARE, nullptr,\n    flags, levels, 2, D3D11_SDK_VERSION, &scd, &swap, &device, nullptr, &ctx);',
    'Direct3D 11 renderer — not available outside Windows.',
    { macos: 'SDL_GPU (Metal backend), or Vulkan via MoltenVK', linux: 'SDL_GPU (Vulkan backend) or DXVK for a quick port', freebsd: 'SDL_GPU (Vulkan backend)' }),
  f(4, 'src/render/shaders/sprite.hlsl', 1, 'HLSL shader model 5', 'graphics', 'high',
    'float4 PSMain(VSOut i) : SV_Target {\n    return tex.Sample(samp, i.uv) * i.color;\n}',
    'HLSL must be cross-compiled for Metal / SPIR-V.',
    { macos: 'Compile with SDL_shadercross or DXC → SPIR-V → MSL', linux: 'DXC → SPIR-V', freebsd: 'DXC → SPIR-V' }),
  f(5, 'src/audio/xaudio2_mixer.cpp', 17, 'XAudio2Create', 'audio', 'high',
    'XAudio2Create(&xaudio, 0, XAUDIO2_DEFAULT_PROCESSOR);', 'XAudio2 is Windows-only.',
    { macos: 'SDL3 audio streams or miniaudio (CoreAudio)', linux: 'SDL3 audio or miniaudio (PipeWire/Pulse/ALSA)', freebsd: 'SDL3 audio or miniaudio (OSS/sndio)' }),
  f(6, 'src/platform/win32_input.cpp', 52, 'XInputGetState', 'input', 'medium',
    'if (XInputGetState(i, &state) == ERROR_SUCCESS) { ... }', 'Gamepad input through XInput.',
    { macos: 'SDL_Gamepad (GameController.framework)', linux: 'SDL_Gamepad (evdev)', freebsd: 'SDL_Gamepad (evdev/usbhid)' }),
  f(7, 'src/core/timer.cpp', 9, 'QueryPerformanceCounter', 'timing', 'low',
    'QueryPerformanceCounter(&now);\nreturn double(now.QuadPart) / freq.QuadPart;', 'High-resolution timer.',
    { macos: 'std::chrono::steady_clock (auto-rewritten)', linux: 'std::chrono::steady_clock (auto-rewritten)', freebsd: 'std::chrono::steady_clock (auto-rewritten)' }),
  f(8, 'src/core/threads.cpp', 30, 'CreateThread / CRITICAL_SECTION', 'threading', 'medium',
    'HANDLE t = CreateThread(nullptr, 0, &JobMain, this, 0, nullptr);\nEnterCriticalSection(&queueLock);',
    'Win32 threading primitives.', { macos: 'std::thread + std::mutex', linux: 'std::thread + std::mutex', freebsd: 'std::thread + std::mutex' }),
  f(9, 'src/core/config_registry.cpp', 14, 'RegOpenKeyExW', 'registry', 'high',
    'RegOpenKeyExW(HKEY_CURRENT_USER, L"Software\\\\EmberQuest", 0, KEY_READ, &key);',
    'Settings stored in the Windows Registry.',
    { macos: 'JSON file in ~/Library/Application Support/EmberQuest', linux: 'JSON file in $XDG_CONFIG_HOME/emberquest', freebsd: 'JSON file in $XDG_CONFIG_HOME/emberquest' }),
  f(10, 'src/core/filesystem.cpp', 21, 'SHGetFolderPathW / backslash paths', 'filesystem', 'medium',
    'swprintf(path, L"%s\\\\saves\\\\slot%d.sav", appData, slot);', 'Windows-specific paths and separators.',
    { macos: 'std::filesystem + SDL_GetPrefPath', linux: 'std::filesystem + SDL_GetPrefPath', freebsd: 'std::filesystem + SDL_GetPrefPath' }),
  f(11, 'src/main.cpp', 3, 'WinMain', 'process', 'medium',
    'int WINAPI wWinMain(HINSTANCE hInst, HINSTANCE, PWSTR, int nShow)', 'Windows-only entry point.',
    { macos: 'int main(int, char**) via SDL_main', linux: 'int main(int, char**)', freebsd: 'int main(int, char**)' }),
  f(12, 'src/ui/hud.cpp', 77, 'MessageBoxW', 'ui', 'low', 'MessageBoxW(nullptr, L"Save failed", L"Error", MB_ICONERROR);',
    'Native dialog box.', { macos: 'SDL_ShowSimpleMessageBox', linux: 'SDL_ShowSimpleMessageBox', freebsd: 'SDL_ShowSimpleMessageBox' }),
  f(13, 'tools/pack_assets.bat', 1, 'Batch script', 'build', 'low', '@echo off\nxcopy /E /Y assets build\\assets', 'Build step written in cmd.exe batch.',
    { macos: 'Converted to a portable CMake step', linux: 'Converted to a portable CMake step', freebsd: 'Converted to a portable CMake step' }),
  f(14, 'EmberQuest.vcxproj', 1, 'MSBuild project', 'build', 'medium', '<Project DefaultTargets="Build" ToolsVersion="17.0">',
    'Visual Studio-only build.', { macos: 'Generated CMakeLists.txt + presets', linux: 'Generated CMakeLists.txt + presets', freebsd: 'Generated CMakeLists.txt + presets' }),
  f(15, 'src/render/d3d11_pipeline.cpp', 64, '__declspec(align(16))', 'other', 'info', 'struct __declspec(align(16)) CBuffer { ... };',
    'MSVC-only attribute.', { macos: 'alignas(16) (auto-rewritten)', linux: 'alignas(16) (auto-rewritten)', freebsd: 'alignas(16) (auto-rewritten)' }),
];

const SCAN: ScanReport = {
  root: '~/Projects/EmberQuest', name: 'EmberQuest', detectedOs: ['windows'],
  languages: [
    { name: 'C++', files: 64, lines: 21840 }, { name: 'HLSL', files: 9, lines: 1210 },
    { name: 'C', files: 6, lines: 2310 }, { name: 'Lua', files: 22, lines: 3480 }, { name: 'Batch', files: 3, lines: 61 },
  ],
  kinds: [
    { name: 'MSBuild', flavor: 'game', evidence: 'EmberQuest.vcxproj, EmberQuest.sln' },
    { name: 'Direct3D 11', flavor: 'game', evidence: 'd3d11.h in 7 files' },
    { name: 'Lua scripting', flavor: 'game', evidence: 'scripts/*.lua, lua_State usage' },
  ],
  totalFiles: 104, totalLines: 28901, portability: 78, findings: FINDINGS,
  categories: { windowing: 14, graphics: 31, audio: 6, input: 5, timing: 4, threading: 7, registry: 3, filesystem: 9, process: 2, ui: 3, build: 4, other: 2 },
  dependencies: [
    { name: 'DirectXTK', source: 'vcpkg.json', portable: false, note: 'Replace with SDL_GPU helpers' },
    { name: 'lua 5.4', source: 'vcpkg.json', portable: true, note: 'Builds everywhere' },
    { name: 'stb_image', source: 'third_party/', portable: true, note: 'Header-only, portable' },
    { name: 'xinput.lib', source: 'EmberQuest.vcxproj', portable: false, note: 'Replaced by SDL_Gamepad' },
  ],
  durationMs: 1840,
};

const QUESTIONS: Question[] = [
  { id: 'strategy', title: 'How should Crossfire port this project?', description: 'This decides how much of your code is rewritten.', kind: 'single', default: 'layer',
    options: [
      { value: 'layer', label: 'Portability layer', description: 'Route OS calls through SDL3. One codebase, every OS.', recommended: true },
      { value: 'native', label: 'Native per OS', description: 'Separate Win32 / Cocoa / X11-Wayland backends. Most native, most code.' },
      { value: 'translate', label: 'Translation layer', description: 'Keep DirectX; ship with DXVK / MoltenVK / GPTK. Fastest, least native.' },
      { value: 'hybrid', label: 'Hybrid', description: 'SDL3 for platform, keep a native renderer where it matters.' },
    ] },
  { id: 'graphics', title: 'Graphics backend', description: 'Direct3D 11 was detected in 7 files.', kind: 'single', default: 'sdl_gpu',
    options: [
      { value: 'sdl_gpu', label: 'SDL_GPU', description: 'Metal on macOS, Vulkan on Linux/FreeBSD, D3D12 on Windows.', recommended: true },
      { value: 'vulkan', label: 'Vulkan + MoltenVK', description: 'One explicit API everywhere.' },
      { value: 'opengl', label: 'OpenGL 4.1', description: 'Simple and widely supported; deprecated on macOS.' },
      { value: 'native', label: 'Native per OS', description: 'D3D12 / Metal / Vulkan, written separately.' },
    ] },
  { id: 'audio', title: 'Audio backend', description: 'XAudio2 was detected.', kind: 'single', default: 'sdl',
    options: [
      { value: 'sdl', label: 'SDL3 audio', description: 'Comes with the portability layer.', recommended: true },
      { value: 'miniaudio', label: 'miniaudio', description: 'Single-file, every backend including OSS/sndio.' },
      { value: 'openal', label: 'OpenAL Soft', description: '3D positional audio.' },
    ] },
  { id: 'buildSystem', title: 'Build system', description: 'Your project uses MSBuild (.vcxproj), which only works with Visual Studio.', kind: 'single', default: 'cmake',
    options: [
      { value: 'cmake', label: 'Generate CMake', description: 'Works with VS, Xcode, Ninja and Make.', recommended: true },
      { value: 'meson', label: 'Generate Meson', description: 'Fast and simple.' },
      { value: 'keep', label: 'Keep MSBuild + add CMake', description: 'Keep the .vcxproj for Windows.' },
    ] },
  { id: 'fidelity', title: 'Rewrite style', description: 'How closely should converted code follow your original?', kind: 'single', default: 'faithful',
    options: [
      { value: 'faithful', label: 'Faithful', description: 'Minimal changes, same structure and names.', recommended: true },
      { value: 'idiomatic', label: 'Idiomatic', description: 'Modern C++20 and clearer code where it helps.' },
    ] },
  { id: 'packages', title: 'Package outputs', description: 'What each build should produce.', kind: 'multi', default: ['exe', 'app', 'appimage', 'pkg'],
    options: [
      { value: 'exe', label: 'Windows .exe + installer', description: 'Portable zip and NSIS installer.' },
      { value: 'app', label: 'macOS .app + .dmg', description: 'Universal (arm64 + x86_64) bundle.' },
      { value: 'appimage', label: 'Linux AppImage', description: 'One file, runs on most distros.' },
      { value: 'flatpak', label: 'Linux Flatpak', description: 'For Flathub and Steam Deck desktop.' },
      { value: 'pkg', label: 'FreeBSD pkg', description: 'Native package for pkg install.' },
    ] },
  { id: 'ci', title: 'Generate a CI workflow', description: 'A GitHub Actions file that builds all four platforms on every push.', kind: 'toggle', default: true, options: [] },
  { id: 'useAi', title: 'Use AI for the hardest files', description: 'Files with blocker findings get a full rewrite by your AI model; everything else uses deterministic rules.', kind: 'toggle', default: true, options: [] },
];

const TOOLCHAINS: Toolchain[] = [
  { id: 'clang', name: 'Apple Clang', found: true, version: '19.0.0', path: '/usr/bin/clang', targets: ['macos'], hint: '' },
  { id: 'cmake', name: 'CMake', found: true, version: '4.2.1', path: '/opt/homebrew/bin/cmake', targets: ['macos'], hint: '' },
  { id: 'xcode', name: 'Xcode', found: true, version: '27.0', path: '/Applications/Xcode.app', targets: ['macos'], hint: '' },
  { id: 'zig', name: 'Zig (cross-compiler)', found: false, targets: ['windows', 'linux', 'freebsd'], hint: 'brew install zig' },
  { id: 'mingw', name: 'MinGW-w64', found: false, targets: ['windows'], hint: 'brew install mingw-w64' },
  { id: 'gptk', name: 'Game Porting Toolkit', found: true, version: '3.0', path: '/Applications/Game Porting Toolkit.app', targets: [], hint: 'Runs Windows builds on this Mac for testing' },
  { id: 'docker', name: 'Docker', found: false, targets: ['linux'], hint: 'brew install --cask docker' },
];

export function mockEngine(): Engine {
  return {
    kind: 'demo',
    hostOs: async () => 'macos',
    pickFolder: async () => '~/Projects/EmberQuest',
    pickFile: async () => '~/Crossfire/EmberQuest-ported/build/windows/EmberQuest.exe',
    openPath: async () => {},
    async scan() {
      const steps = 34;
      for (let i = 0; i <= steps; i++) {
        emit({ stage: 'scan', message: i < steps ? 'Reading source' : 'Scan complete', progress: i / steps, file: FILES[i % FILES.length] });
        await wait(70);
      }
      return SCAN;
    },
    questions: async () => QUESTIONS,
    async convert(req) {
      const log: [string, ProgressEvent['level']?][] = [
        ['Copying project to output folder'], ['Generating crossfire/cf_platform.h shim'],
        ['Rewriting QueryPerformanceCounter → std::chrono (4 sites)', 'success'], ['Rewriting CreateThread → std::thread (7 sites)', 'success'],
        ['Rewriting __declspec(align) → alignas (2 sites)', 'success'], ['Replacing registry config with JSON in pref path', 'success'],
        ['AI: rewriting src/platform/win32_window.cpp → platform/sdl_window.cpp'], ['AI: rewriting src/render/d3d11_device.cpp → render/gpu_device.cpp'],
        ['AI: confidence 0.82 on gpu_device.cpp — flagged for review', 'warn'], ['Cross-compiling HLSL → SPIR-V / MSL via shadercross'],
        ['Generating CMakeLists.txt + CMakePresets.json'], ['Generating .github/workflows/crossfire-build.yml'],
        ['Writing CROSSFIRE_REPORT.md'], [`Done — ported to ${req.targets.length} targets`, 'success'],
      ];
      for (let i = 0; i < log.length; i++) {
        emit({ stage: 'convert', message: log[i][0], level: log[i][1] ?? 'info', progress: (i + 1) / log.length });
        await wait(380);
      }
      const out: ConvertResult = {
        output: req.output, reportPath: `${req.output}/CROSSFIRE_REPORT.md`, durationMs: 5320, manualReview: FINDINGS.filter((x) => x.severity === 'blocker'),
        changes: [
          { path: 'crossfire/cf_platform.h', action: 'added', method: 'template', confidence: 1, notes: ['Portable sleep, ticks, paths, string helpers'] },
          { path: 'CMakeLists.txt', action: 'added', method: 'template', confidence: 1, notes: ['SDL3 via FetchContent; macOS bundle; FreeBSD section'] },
          { path: '.github/workflows/crossfire-build.yml', action: 'added', method: 'template', confidence: 1, notes: ['windows / macos / ubuntu / freebsd-vm'] },
          { path: 'src/platform/sdl_window.cpp', action: 'modified', method: 'ai', confidence: 0.91, notes: ['Replaced Win32 window + WndProc with SDL3'],
            diff: '@@ -38,12 +38,9 @@\n-HWND hwnd = CreateWindowExW(0, L"EmberQuest", title, WS_OVERLAPPEDWINDOW,\n-    CW_USEDEFAULT, CW_USEDEFAULT, w, h, nullptr, nullptr, hInst, nullptr);\n-if (!hwnd) return false;\n-ShowWindow(hwnd, nShow);\n+SDL_Window* window = SDL_CreateWindow(title, w, h, SDL_WINDOW_RESIZABLE | SDL_WINDOW_HIGH_PIXEL_DENSITY);\n+if (!window) return false;\n@@ -85,8 +82,7 @@\n-while (PeekMessageW(&msg, nullptr, 0, 0, PM_REMOVE)) {\n-    TranslateMessage(&msg);\n-    DispatchMessageW(&msg);\n-}\n+for (SDL_Event e; SDL_PollEvent(&e);) {\n+    handle_event(e);\n+}' },
          { path: 'src/render/gpu_device.cpp', action: 'modified', method: 'ai', confidence: 0.82, notes: ['D3D11 → SDL_GPU; review blend states'],
            diff: '@@ -20,9 +20,6 @@\n-hr = D3D11CreateDeviceAndSwapChain(nullptr, D3D_DRIVER_TYPE_HARDWARE, nullptr,\n-    flags, levels, 2, D3D11_SDK_VERSION, &scd, &swap, &device, nullptr, &ctx);\n-if (FAILED(hr)) return false;\n+gpu = SDL_CreateGPUDevice(SDL_GPU_SHADERFORMAT_SPIRV | SDL_GPU_SHADERFORMAT_MSL | SDL_GPU_SHADERFORMAT_DXIL, debug, nullptr);\n+if (!gpu || !SDL_ClaimWindowForGPUDevice(gpu, window)) return false;' },
          { path: 'src/core/timer.cpp', action: 'modified', method: 'rule', confidence: 0.99, notes: ['QueryPerformanceCounter → steady_clock'],
            diff: '@@ -7,4 +7,3 @@\n-LARGE_INTEGER now;\n-QueryPerformanceCounter(&now);\n-return double(now.QuadPart) / freq.QuadPart;\n+using clock = std::chrono::steady_clock;\n+return std::chrono::duration<double>(clock::now().time_since_epoch()).count();' },
          { path: 'src/core/threads.cpp', action: 'modified', method: 'rule', confidence: 0.97, notes: ['CreateThread → std::thread, CRITICAL_SECTION → std::mutex'] },
          { path: 'src/core/config_registry.cpp', action: 'modified', method: 'rule', confidence: 0.93, notes: ['Registry → JSON in SDL_GetPrefPath'] },
          { path: 'src/audio/sdl_mixer.cpp', action: 'modified', method: 'ai', confidence: 0.88, notes: ['XAudio2 → SDL3 audio streams'] },
          { path: 'src/render/shaders/sprite.spv / .msl', action: 'added', method: 'template', confidence: 1, notes: ['Cross-compiled from HLSL'] },
          { path: 'assets/**', action: 'copied', method: 'copy', confidence: 1, notes: ['1,204 files'] },
        ],
      };
      return out;
    },
    toolchains: async () => TOOLCHAINS,
    async build(req) {
      const results: BuildResult['results'] = [];
      for (const t of req.targets) {
        emit({ stage: 'build', message: `[${t}] configuring`, progress: -1 });
        await wait(500);
        emit({ stage: 'build', message: `[${t}] compiling 97 translation units`, progress: -1 });
        await wait(700);
        if (t === 'macos') {
          emit({ stage: 'build', message: '[macos] EmberQuest.app signed (ad-hoc)', level: 'success', progress: -1 });
          results.push({ target: t, status: 'success', artifacts: ['build/macos/EmberQuest.app', 'build/macos/EmberQuest.dmg'], log: '' });
        } else {
          emit({ stage: 'build', message: `[${t}] no local cross-compiler — queued for CI`, level: 'warn', progress: -1 });
          results.push({ target: t, status: 'ci', artifacts: [], log: '', reason: 'Install zig to build locally, or push to GitHub to build in CI.' });
        }
      }
      return { results, ciWorkflow: '.github/workflows/crossfire-build.yml' };
    },
    async testWithGptk() {
      for (const m of ['Creating Wine prefix', 'Launching through D3DMetal', 'Window created 1280×720', 'Exited cleanly after 12s']) {
        emit({ stage: 'test', message: m, progress: -1 });
        await wait(500);
      }
      return 'Ran 12s under Game Porting Toolkit, exit code 0.';
    },
    onProgress: (cb) => { listeners.add(cb); return () => listeners.delete(cb); },
  };
}

export const ALL_OS: Os[] = ['windows', 'macos', 'linux', 'freebsd'];
