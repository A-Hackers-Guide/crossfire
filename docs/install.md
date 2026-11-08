# Install

## Prebuilt binaries

Get the build for your OS from the latest [release](https://github.com/A-Hackers-Guide/crossfire/releases), or from the **Build Crossfire** workflow run on `main`.

| OS | Main app (Tauri) | Backup app (Electron) |
|---|---|---|
| Windows | `.exe` installer / `.msi` | `.exe` installer + portable `.exe` |
| macOS | `.dmg` / `.app` (universal) | `.dmg` |
| Linux | `.AppImage` / `.deb` / `.rpm` | `.AppImage` |
| FreeBSD | binary tarball (`crossfire` + `crossfire-engine`) | — |

The macOS `.app` is universal (`aarch64` + `x86_64`) so it runs on both Apple silicon and Intel without a separate download.

## Engine only

The engine ships both as part of the app bundles and as a standalone CLI. Grab `crossfire-engine` from the same release page if you just want the command-line workflow.

```bash
# Linux / macOS
chmod +x crossfire-engine
./crossfire-engine --help
```

```powershell
# Windows
.\crossfire-engine.exe --help
```

## Build from source

```bash
git clone https://github.com/A-Hackers-Guide/crossfire.git
cd crossfire

# Engine only (Rust 1.80+)
cargo build --manifest-path engine/Cargo.toml --release

# Full Tauri app (requires Node 22 + platform SDKs)
npm --prefix ui ci
npm --prefix apps/tauri ci
cd apps/tauri && npx tauri build
```

See the generated per-OS prerequisites in the `tauri` job of `.github/workflows/build.yml` for the exact system libraries each platform needs.

## Supported hosts

| Host OS | Builds locally | Builds in CI |
|---|---|---|
| macOS 13+ | all four targets with GPTK / zig cross | yes |
| Linux | Linux + Windows (MinGW) + FreeBSD (zig) | yes |
| Windows | Windows native | yes |
| FreeBSD | FreeBSD native | yes |

Anything your host can't produce locally still builds through the generated GitHub Actions workflow in your converted project.
