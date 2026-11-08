# Generated CMake + CI

Convert writes a complete build + release pipeline into the output directory. Nothing you need to add; you can start a PR against the converted project immediately.

## What gets written

```
<output>/
├── CMakeLists.txt
├── CMakePresets.json
├── crossfire/
│   ├── cf_platform.h
│   └── cf_platform.c
├── packaging/
│   ├── windows/   # wix + nsis bits
│   ├── macos/     # Info.plist, entitlements, codesign scripts
│   ├── linux/     # AppImage + deb + rpm specs
│   └── freebsd/   # pkg manifest
├── .github/
│   └── workflows/
│       └── build.yml  # builds all four OSes
└── CROSSFIRE_REPORT.md
```

## `CMakePresets.json`

Presets you can pick with `--preset <name>`:

| Preset | Compiler | Target OS |
|---|---|---|
| `windows-msvc` | MSVC | Windows |
| `windows-clang-cl` | clang-cl | Windows |
| `windows-mingw` | MinGW GCC | Windows (cross-compile from Linux/macOS) |
| `macos-arm64` | AppleClang | macOS Apple silicon |
| `macos-x86_64` | AppleClang | macOS Intel |
| `macos-universal` | AppleClang | macOS fat (default on macOS) |
| `linux-gcc` | GCC | Linux |
| `linux-clang` | Clang | Linux |
| `freebsd` | Clang | FreeBSD |

## Generated CI

The generated `.github/workflows/build.yml` matches the preset set: one job per preset, caching enabled, releases uploaded on tags. It uses the same pattern as Crossfire's own build workflow — so if you already like Crossfire's release flow, you get it for free in your converted projects.

## Overriding

Everything in the generated output is normal source: edit `CMakeLists.txt` or `build.yml` the way you'd edit any CMake project. The engine emits a `CROSSFIRE_MANIFEST.json` listing every generated file so a subsequent re-run can tell what's yours vs. what's generated.
