# `crossfire-engine`

The engine is a standalone binary. Everything the GUI does, you can do from the CLI.

```text
crossfire-engine <COMMAND> [OPTIONS]
```

## Commands

| Command | What it does |
|---|---|
| `scan <PATH>` | Walk `PATH` and emit a JSON `ScanReport` on stdout |
| `questions` | Read a scan from stdin, emit tailored questions for the chosen targets |
| `convert` | Read a `ConvertRequest` from stdin, write the port to disk, emit a `ConvertResult` |
| `build` | Read a `BuildRequest` from stdin, build each target, emit progress + a `BuildResult` |
| `toolchains` | Enumerate compilers / cross-compilers / build tools / runtimes on this host |
| `gptk <EXE>` | Run a Windows executable through Game Porting Toolkit (macOS) or Wine |
| `rules` | Emit the compiled rule table as JSON (useful for tooling + docs) |
| `host` | Print the OS the engine is running on |

## Common flows

### Just analyze

```bash
crossfire-engine scan ./my-project > scan.json
jq '.score, .findings[].id' scan.json | head
```

### Full port, headless

```bash
crossfire-engine scan ./my-project > scan.json

# Build a ConvertRequest manually or from the GUI's "Export request" button
crossfire-engine convert < convert-request.json
```

### Discover what this machine can build

```bash
crossfire-engine toolchains | jq '.[] | {name, version, path}'
```

### Smoke-test a Windows build on macOS

```bash
crossfire-engine gptk ./dist/MyGame.exe
```

## Output format

Every subcommand emits newline-delimited JSON on stdout. Progress events during long-running commands (`scan`, `convert`, `build`) are sent to stderr as `ProgressEvent` JSON objects:

```json
{"kind":"stage","name":"scan","message":"walking project tree"}
{"kind":"progress","done":17,"total":42,"message":"ui/main.cpp"}
{"kind":"stage","name":"scan","message":"compiling rules"}
```

The final result goes to stdout as a single JSON object.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Runtime error (bad input, I/O failure, toolchain missing) |
| 2 | Usage error |
| 3 | Partial result (e.g. convert finished but one target build failed) — the JSON result still describes what happened |
