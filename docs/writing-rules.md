# Writing a rule

New rules are pure data: add a group or append to an existing group in the right TOML table, run the tests, open a PR.

## Shape

Each table is a list of `[[group]]` blocks. A group sets defaults — language scope, source OS, category, severity, optional fix hint — then lists individual `rules` that override just what they need.

```toml
# engine/src/rules/win32.toml

[[group]]
lang = ["c", "cpp"]
os   = ["windows"]
cat  = "timing"
sev  = "warn"
fix  = "Use std::chrono::steady_clock instead of QueryPerformanceCounter."

[[group.rules]]
id      = "win.timing.qpc"
pattern = '\bQueryPerformanceCounter\s*\('
note    = "Windows-specific high-resolution counter."
```

### Required fields

| Field | What it is |
|---|---|
| `group.lang` | Languages this group applies to (`c`, `cpp`, `cs`, `python`, `go`, `rust`, `java`, `node`, or `any`) |
| `group.os` | Source OSes the pattern is characteristic of (`windows`, `macos`, `linux`, `freebsd`, or multiple) |
| `group.cat` | Category (see [Rule catalog](rules.md)) |
| `group.sev` | Severity (`block` / `warn` / `info`) |
| `rules[].id` | Globally unique identifier. Convention: `<os>.<cat>.<name>` |
| `rules[].pattern` | Regex (Rust `regex` crate dialect) |

### Optional fields

- `group.fix` — a human sentence. Shown in reports and passed to the AI pass.
- `rules[].note` — one-line description.
- `rules[].fix` — rule-specific fix hint that overrides `group.fix`.
- `rules[].replace` — mechanical replacement template (uses regex capture groups).

## Testing a new rule

1. Add a fixture under `engine/tests/fixtures/<platform>/` that would trip the rule.
2. Run `cargo test --manifest-path engine/Cargo.toml`.
3. If the rule carries `replace`, also eyeball the diff in the generated `CROSSFIRE_REPORT.md` to make sure the rewrite is what you wanted.

## Style

- Favor **narrow regexes** over broad ones. `\bQueryPerformanceCounter\s*\(` rather than just `QueryPerformanceCounter`.
- Use `(?i)` only when the API really is case-insensitive (it almost never is on Windows C).
- Prefer `block` only when the code genuinely cannot build on the target. Most APIs that *can* be replaced should be `warn`.
- Keep `note` under ~80 chars.

## Running the lint gates

The repo runs `cargo fmt --check` and `cargo clippy -- -D warnings` on every PR; both must pass. There are also `oxlint` and `tsc` gates on the UI, and `cargo-deny` on the engine supply chain. See `.github/workflows/lint.yml`.
