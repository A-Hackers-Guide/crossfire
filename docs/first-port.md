# Your first port

Walk-through for porting a Windows-only project to Linux + macOS, from zero to a build.

## 1. Pick the source

Launch Crossfire. On the **Source** tab, pick the project folder.

If you already know what it targets (e.g. "Windows, DirectX 11"), say so on the drop-downs. Otherwise leave them as "Detect" and Crossfire figures it out from the code.

## 2. Analyze

Hit **Analyze**. The engine walks every file through 599 rules, scoring portability and tagging every OS-specific call with:

- the file and line
- the category (window, audio, graphics, registry, filesystem, threading, timing, AI, engine, …)
- the severity (block, warn, info)
- a suggested replacement per target OS

The **Analyze** tab shows a heat map of where the project touches the OS and a score 0 – 100.

## 3. Choose targets

On the **Targets** tab, tick the OSes you want. Anything you don't tick is skipped by the convert step.

## 4. Configure

The **Configure** tab builds its question set from what Analyze found. Typical questions:

- **Porting strategy.** SDL3 portability layer (recommended default), native per OS, translation layer (Wine / Proton / GPTK for one-way), hybrid.
- **Graphics backend.** DirectX → Vulkan, Metal, OpenGL, WebGPU, or a passthrough translation layer.
- **Audio backend.** XAudio2 / WASAPI / ALSA / PulseAudio / Core Audio / miniaudio.
- **Build system.** CMake, Meson, Bazel.
- **C++ standard.** 17 / 20 / 23.
- **Packaging.** `.msi` / `.dmg` / `.deb` / `.rpm` / AppImage / tarball.
- **CI.** GitHub Actions (default), GitLab CI, none.
- **AI.** Off / Claude / OpenAI-compatible endpoint (including local models).

Defaults are chosen based on the scan — e.g. if the project is DirectX-heavy and you picked Linux, graphics defaults to Vulkan.

## 5. Convert

**Convert** writes the output project to a fresh directory. Four things happen:

1. **Mechanical rule-based rewrites.** `Sleep` → portable sleep, `QueryPerformanceCounter` → `std::chrono`, `__declspec(align)` → `alignas`, path separators, include guards, …
2. **Portability shim** (`crossfire/cf_platform.h`) with thin inline wrappers.
3. **Build + CI templates.** `CMakeLists.txt`, `CMakePresets.json`, packaging scripts, a GitHub Actions workflow that builds all four targets.
4. **AI pass**, if enabled, over the hardest files, with diffs and confidence scores.

The original source is never modified. Every change is listed in `CROSSFIRE_REPORT.md` in the output directory.

## 6. Build

Hit **Build**. Crossfire runs each target it has the toolchain for — native compilers, MinGW cross-compile, zig cross-compile. The rest go to CI with the generated workflow.

## 7. Test (macOS only, for now)

On a Mac, the **Test** tab runs the Windows binary through Apple's Game Porting Toolkit directly from the app. You'll see stderr/stdout inline and whether GPTK had to translate any API calls at runtime.
