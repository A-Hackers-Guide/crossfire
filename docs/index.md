# Crossfire

**Port any project between Windows, macOS, Linux, and FreeBSD.**

Point Crossfire at your source code. It maps every OS-specific call, asks a few questions, writes a portable version, and builds it — without touching the original.

## In one picture

1. **Source** — pick a project folder.
2. **Analyze** — 674 rules scan for Win32, DirectX, Cocoa/Metal, X11/Wayland, ALSA/PulseAudio, kqueue/epoll, registry, filesystem paths, threading, timing, C#/Python/Go/Rust/Java/Node, and the big game engines.
3. **Targets** — tick the OSes you want to support.
4. **Configure** — questions appear based on what was found: porting strategy (SDL3 portability layer, native per OS, translation layer, hybrid), graphics and audio backends, build system, C++ standard, packaging, CI, AI.
5. **Convert** — writes a new copy. Mechanical rewrites + a portability shim + generated CMake + packaging + a GitHub Actions workflow that builds all four OSes. Optional AI pass for the hard files.
6. **Build** — compiles each target this machine can produce. The rest build in CI.
7. **Test** — on macOS, run the Windows build through Apple's Game Porting Toolkit from the app.

## What this is

- A **GUI app** (Tauri + Electron builds in Releases) **and** a **standalone engine binary** (`crossfire-engine`).
- **Read-only** on the input. Convert writes to a separate output tree.
- **No cloud required.** All rule-based rewrites run locally. The AI pass is opt-in and supports any OpenAI-compatible endpoint, including local models.
- **MIT licensed.**

## Start here

- [Install](install.md)
- [First port](first-port.md)
- [CLI reference](cli.md)
- [Rule catalog](rules.md)

## Repo links

- Source: <https://github.com/A-Hackers-Guide/crossfire>
- Releases: <https://github.com/A-Hackers-Guide/crossfire/releases>
- Issues: <https://github.com/A-Hackers-Guide/crossfire/issues>
