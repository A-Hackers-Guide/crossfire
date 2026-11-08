# FAQ

### Does it modify my source?

No. Convert writes to a fresh output directory. The input tree is opened read-only. The report file (`CROSSFIRE_REPORT.md`) lives in the output.

### Is the AI required?

No. The rule-based pass produces a working port for most projects on its own. AI is opt-in and only sees files the rules couldn't handle cleanly.

### Where does my API key go?

In `CROSSFIRE_AI_KEY` at convert time. Crossfire never stores it in the project file or logs. It's held in memory for the duration of the convert and discarded after.

### Does it support FreeBSD?

Yes. FreeBSD is a first-class target for convert and for packaging. The GUI runs via the binary tarball; the engine CLI runs anywhere Rust runs.

### Can I run only the engine?

Yes — `crossfire-engine` is a standalone binary that does everything the GUI does, with JSON in and JSON out. See [CLI reference](cli.md).

### What C++ standards are supported?

17, 20, and 23. 17 is the safe default; 20 enables the shim's `concepts`-flavored overloads; 23 enables `std::expected` in error-returning helpers.

### Does it work on games?

Yes — that's much of the design motivation. There are rule groups for Unity, Unreal, Godot, and Love2D in `engines.toml`. The DirectX / graphics rewrite paths are targeted at games.

### What happens to Win32 features without a Linux equivalent?

Two options at configure time:

- **Translation layer** (Wine / Proton / GPTK) — keep the Windows binary semantics, let a layer forward calls.
- **Stub + warn** — replace the call with a stub that does the closest-matching thing on the target, log a warning, and surface it in the convert report.

You can mix these per feature (window handling: translation, audio: native, etc.).

### Can I add my own rules without a fork?

Yes. Drop a TOML file in `~/.config/crossfire/rules/` (or `%APPDATA%\crossfire\rules\` on Windows) and the engine picks it up at launch alongside the built-ins. The schema is the same as the built-in tables — see [Writing a rule](writing-rules.md).

### How big is the port I should expect?

Depends entirely on the project. Analyze gives you a **portability score** before you run Convert; a score above 70 is usually a mechanical port with no code review needed, 40-70 needs some human eyes on the stubbed-out bits, below 40 is "worth considering SDL3-first instead".

### Why is it called Crossfire?

Cross-OS portability. The target list kept crossing and recrossing OS boundaries. The icon is in `assets/icon.png`.
