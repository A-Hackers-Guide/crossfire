# Architecture

Crossfire is two pieces: a Rust **engine** and a **GUI shell** (Tauri main / Electron backup).

```
┌─────────────────────────────────────────┐
│  UI  ─  React + TypeScript + Vite       │  ui/
└─────────────────────────────────────────┘
          │  bridge.ts  (Tauri commands)
┌─────────────────────────────────────────┐
│  Tauri shell                            │  apps/tauri
│  — thin, mostly dispatches to engine    │
└─────────────────────────────────────────┘
          │  crate crossfire_engine
┌─────────────────────────────────────────┐
│  Engine — Rust library + CLI           │  engine/
│                                         │
│   scan.rs       walk + match rules      │
│   rules/        9 TOML tables           │
│   lang.rs       language detection      │
│   detect.rs     project type detection  │
│   questions.rs  configure step logic    │
│   convert.rs    the writer              │
│   rewrite.rs    deterministic rewrites  │
│   shim.rs       cf_platform.* emission  │
│   templates.rs  CMake + CI + packaging  │
│   ai.rs         optional AI pass        │
│   toolchain.rs  host toolchain scan     │
│   build.rs      per-target build        │
│   gptk.rs       GPTK / Wine runner      │
│   stubs.rs      per-OS stub generation  │
│   model.rs      shared types (serde)    │
│   util.rs       path + regex helpers    │
└─────────────────────────────────────────┘
```

## Why Rust + Tauri (+ Electron backup)

- **Rust engine**: deterministic, embeddable, one binary per OS, same code on CLI and GUI, easy to fuzz and test.
- **Tauri**: small installers, one webview per target, native menu + file dialogs, no bundled Chromium.
- **Electron backup**: for the small set of hosts where Tauri's webview behaviour is a problem (ancient Windows, older distros without a working webkit2gtk, headless CI smoke-testing of the UI itself).

The UI never calls OS APIs directly. Every operation goes through a Tauri command that forwards to the engine.

## Progress callbacks

Long-running engine functions (`scan`, `convert`, `build`) take an `on: &dyn Fn(ProgressEvent)` callback. The Tauri layer forwards those events to a channel the UI subscribes to. The CLI subcommands print the same events as newline-delimited JSON on stderr.

## Serialization

Everything that crosses the engine ↔ UI boundary is `serde`-derived with explicit `rename_all = "camelCase"`:

- `ScanReport` — the output of `scan`
- `Question` + `Answer` — the configure step
- `ConvertRequest` / `ConvertResult`
- `BuildRequest` / `BuildResult`
- `Toolchain`, `Finding`, `Os`, `Category`, `Severity`, `AiProvider`

The serialized JSON is also the stable contract for the CLI — if you drive the engine from scripts, those shapes are what you consume.

## Rules as data

All 705 rules live in TOML, not in code. Adding a rule is a two-line PR plus a fixture. See [Writing a rule](writing-rules.md).

## What's not here

- **Not a Wine/Proton replacement.** Translation layer is one of four porting strategies; the default is a proper native port.
- **Not a sandbox.** The engine runs trusted code (compilers, build tools). It does not try to contain what your build does.
- **Not a security scanner.** Findings are about **portability**, not CVEs.
