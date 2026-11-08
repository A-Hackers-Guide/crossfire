# Rule catalog

Crossfire's engine ships with 705 rules in nine TOML tables. Each rule carries:

- a **regex** that matches the offending code
- a **language scope** (C / C++ / C# / Python / Go / Rust / Java / Node / glob)
- the **source OS** it's tied to
- a **category** (window / audio / graphics / registry / filesystem / threading / timing / language / engine / build)
- a **severity** (block / warn / info)
- an optional **fix** hint, per target OS

## Table layout

Rules live under `engine/src/rules/*.toml`:

| Table | Scope |
|---|---|
| `win32.toml` | Windows API, kernel32, user32, advapi32, registry, COM |
| `directx.toml` | Direct3D 9/10/11/12, DXGI, DirectInput, XAudio, DirectCompute |
| `macos.toml` | Cocoa, AppKit, Core Audio / Graphics / Image / Foundation, Metal, kqueue |
| `linux.toml` | X11, Wayland, GTK, Qt, ALSA, PulseAudio, systemd, inotify, epoll |
| `bsd.toml` | BSD-specific syscalls, jail, procfs quirks, rc.d |
| `posix.toml` | Portable POSIX code: shared between Linux / macOS / BSD |
| `languages.toml` | Language-level APIs: C# / Python / Go / Rust / Java / Node |
| `engines.toml` | Unity, Unreal, Godot, Love2D specifics |
| `build.toml` | Build systems: CMake, Meson, MSBuild, Makefile, Cargo, npm |

## Severity

| Severity | Meaning | Default conversion behavior |
|---|---|---|
| `block` | Code **will not build or run** on the target without a replacement | Convert replaces it mechanically, or stops and asks |
| `warn` | Code will build but behave differently or degrade | Convert replaces it mechanically if possible, else notes it |
| `info` | Idiomatic difference; works but could be cleaner | Convert leaves the code, surfaces it in `CROSSFIRE_REPORT.md` |

## Category

Findings are grouped by category in the Analyze heat map. The vocabulary:

- `window` — OS windowing (HWND, NSWindow, X Window, wl_surface)
- `audio` — mixers and output (XAudio2, WASAPI, ALSA, PulseAudio, CoreAudio)
- `graphics` — rendering APIs (Direct3D, Metal, OpenGL, Vulkan)
- `input` — HID and gamepad (DirectInput, GameController, SDL, …)
- `registry` — Windows registry
- `filesystem` — path separators, `\\` prefixes, case sensitivity, `/proc` quirks
- `threading` — Win32 threads vs pthreads vs C++11, TLS, affinity
- `timing` — QPC, mach_absolute_time, clock_gettime, std::chrono
- `language` — language-standard-library APIs (`System.Threading` in C#, `winreg` in Python, …)
- `engine` — Unity / Unreal / Godot-specific platform hooks
- `build` — build system platform assumptions

## Dumping the table

```bash
crossfire-engine rules | jq '.[] | {id, os, cat, sev}' | head -20
```

You get one JSON object per rule, with its id, source OS, category, severity, and language scope. Handy when you want to diff the catalog between versions, or drive your own tooling.

## Adding a rule

See [Writing a rule](writing-rules.md).
