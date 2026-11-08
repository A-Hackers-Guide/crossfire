# Toolchain detection

`crossfire-engine toolchains` enumerates every compiler, cross-compiler, build tool, and runtime it can find on the host, and reports each as a `Toolchain` JSON object.

## What it looks for

| Kind | Examples |
|---|---|
| Native compilers | MSVC (`cl.exe`), clang-cl, AppleClang (Xcode), system GCC, system Clang |
| Cross-compilers | MinGW (x86_64-w64-mingw32-gcc), zig (`zig cc --target=<triple>`), rustup cross targets |
| Build tools | CMake, Meson, Ninja, GNU Make, MSBuild, xcodebuild, Cargo, npm, Bun |
| Package tools | WiX, NSIS, `pkg` (BSD), `dpkg-deb`, `rpmbuild`, AppImageTool, `codesign` |
| Runtimes | Node, Python, dotnet, Java, Go |
| Translation layers | Game Porting Toolkit (`gptk` binaries + Rosetta), Wine, Proton |

Detection is a best-effort PATH + well-known-locations scan. It does not install anything.

## Output

```json
[
  {
    "kind": "compiler",
    "name": "clang",
    "version": "17.0.6",
    "path": "/usr/bin/clang",
    "targets": ["x86_64-linux-gnu"]
  },
  {
    "kind": "cross-compiler",
    "name": "zig",
    "version": "0.13.0",
    "path": "/usr/local/bin/zig",
    "targets": ["x86_64-windows-gnu", "aarch64-linux-gnu", "x86_64-freebsd"]
  },
  ...
]
```

Fields:

- `kind` — one of `compiler`, `cross-compiler`, `build-tool`, `package-tool`, `runtime`, `translation-layer`
- `name` — canonical name
- `version` — the tool's own `--version` output, parsed where possible
- `path` — absolute path to the executable
- `targets` — for cross-compilers, the target triples it can emit

## Using it in the UI

The **Build** tab shows the matrix of "which targets can this machine produce locally vs. which ones need CI". That matrix is a direct function of `toolchains()` plus the current Configure step's targets.

## Headless

```bash
crossfire-engine toolchains | jq '.[] | select(.kind == "cross-compiler")'
```

...tells you which targets you can cross-compile right now, without opening the UI.
