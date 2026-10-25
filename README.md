<p align="center">
  <img src="assets/icon.png" width="128" alt="Crossfire icon">
</p>

<h1 align="center">Crossfire</h1>

<p align="center"><b>Port any project between Windows, macOS, Linux and FreeBSD.</b><br>
Point it at your source code. It maps every OS-specific call, asks a few questions, writes a portable version, and builds it.</p>

---

## What it does

1. **Source:** pick a project folder. Optionally say which OS it was written for; Crossfire also detects it.
2. **Analyze:** scans every file against **599 rules** covering Win32, DirectX, Cocoa/Metal, X11/Wayland, ALSA/PulseAudio, kqueue/epoll, the registry, filesystem paths, threading, timing, plus language-level APIs (C#, Python, Go, Rust, Java, Node) and game engines (Unity, Unreal, Godot, Love2D). You get a portability score, a heat map of where the code touches the OS, and a suggested replacement for every finding, per target OS.
3. **Targets:** choose any of Windows, macOS, Linux and FreeBSD.
4. **Configure:** questions based on what was found: porting strategy (SDL3 portability layer, native per OS, translation layer, hybrid), graphics and audio backends, build system, C++ standard, packaging, CI, AI assistance.
5. **Convert:** writes a new copy of the project and never touches the original.
   - Mechanical rule-based rewrites (`Sleep` → portable sleep, `QueryPerformanceCounter` → `std::chrono`, `__declspec(align)` → `alignas`, path separators, include guards…)
   - A generated portability shim (`crossfire/cf_platform.h`)
   - Generated `CMakeLists.txt` + presets, packaging scripts, and a GitHub Actions workflow that builds all four OSes
   - Optional **AI pass** (Claude, or any OpenAI-compatible endpoint, including local models) for the hardest files, with diffs and confidence scores
   - `CROSSFIRE_REPORT.md` listing every change and what needs a human to look at
6. **Build:** builds each target this machine can produce (native compilers, MinGW, zig cross-compiling). The rest build in CI with the generated workflow.
7. **Test:** on a Mac, run the Windows build through Apple's **Game Porting Toolkit** right from the app.

## Download

Grab the build for your OS from [Releases](../../releases), or from the latest run of the **Build Crossfire** workflow:

| OS | Main app (Tauri) | Backup app (Electron) |
|---|---|---|
| Windows | `.exe` installer / `.msi` | `.exe` installer + portable `.exe` |
| macOS | `.dmg` / `.app` (universal) | `.dmg` |
| Linux | `.AppImage` / `.deb` / `.rpm` | `.AppImage` |
| FreeBSD | binary tarball (`crossfire` + `crossfire-engine`) | — |

The engine also works on its own from the command line:

```sh
crossfire-engine scan ./MyGame > scan.json
crossfire-engine questions scan.json --targets linux,macos,freebsd
crossfire-engine convert request.json
crossfire-engine build build.json
crossfire-engine toolchains
```

## How it's built

```
crossfire/
├── ui/              React + TypeScript GUI (shared by both shells; runs in demo mode in a browser)
├── engine/          Rust engine: scan, rules, questions, convert, AI, toolchains, build, GPTK
│   └── src/rules/   599 rules in TOML tables
├── apps/tauri/      Main app: Tauri 2, engine linked in-process → one native executable per OS
├── apps/electron/   Backup app: Electron, engine runs as a sidecar CLI
└── .github/workflows/build.yml   Builds every platform + publishes releases on tags
```

Three ways to run the same engine: the Tauri app, the Electron app, and the CLI.

## Developing

Requires Node 22+ and Rust (stable).

```sh
npm --prefix ui install
npm --prefix ui run dev                 # GUI in the browser with the demo engine

cargo test --manifest-path engine/Cargo.toml

cd apps/tauri && npm install && npx tauri dev     # native app
cd apps/electron && npm install && npm start      # backup app
```

On Linux, Tauri needs `libwebkit2gtk-4.1-dev librsvg2-dev patchelf`.

## Honest limits

Crossfire automates the bulk of a port, not 100% of it.

- APIs with no portable equivalent (a closed-source Windows-only `.dll`, kernel drivers, anti-cheat) are flagged, not converted.
- Graphics rewrites (DirectX → SDL_GPU/Vulkan/Metal) and UI-toolkit changes (WinForms → Avalonia, Cocoa → cross-platform) rely on the AI pass and need review.
- Building a macOS app requires a Mac (Apple's SDK license); the CI workflow uses GitHub's macOS runners for this.
- Unity, Unreal and Godot projects are analyzed and converted, but should be exported from their editors.
- Zig and Docker builds are implemented but less tested than native builds.

The report always says what was converted, how confident it is, and what you need to check.

## License

MIT
