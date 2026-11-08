# Contributing

Thanks for helping. This file covers the day-one checklist; the deeper references are in [the docs site](https://a-hackers-guide.github.io/crossfire/).

## Ways to contribute

1. **File a rule request.** Spot a Windows / macOS / Linux / BSD-only API that Crossfire misses? Open a [rule request issue](.github/ISSUE_TEMPLATE/rule.yml) or send the rule directly as a PR — see [Writing a rule](https://a-hackers-guide.github.io/crossfire/writing-rules/).
2. **File a bug.** Use the [bug template](.github/ISSUE_TEMPLATE/bug.yml). If a conversion produced wrong output, attach `CROSSFIRE_REPORT.md` and (if possible) a minimal repro.
3. **Propose a feature.** [Feature template](.github/ISSUE_TEMPLATE/feature.yml) — lead with the use case.
4. **Fix a bug or implement a feature.** Open a PR.

## Setup

```sh
git clone https://github.com/A-Hackers-Guide/crossfire.git
cd crossfire

# UI
npm --prefix ui install
npm --prefix ui run dev                 # GUI in a browser with the demo engine

# Engine
cargo fmt     --manifest-path engine/Cargo.toml
cargo clippy  --manifest-path engine/Cargo.toml --all-targets -- -D warnings
cargo test    --manifest-path engine/Cargo.toml

# Full Tauri app
cd apps/tauri && npm install && npx tauri dev
```

## Pre-flight before opening a PR

The lint workflow runs five gates on every PR:

| Gate | Command |
|---|---|
| rustfmt | `cargo fmt --manifest-path engine/Cargo.toml --check` |
| clippy | `cargo clippy --manifest-path engine/Cargo.toml --all-targets -- -D warnings` |
| cargo-deny | `cargo deny --manifest-path engine/Cargo.toml check` |
| oxlint | `npm --prefix ui run lint` |
| tsc | `(cd ui && npx tsc -b --noEmit)` |

If any of them fail locally, CI will fail too.

## Rules

The rule engine is fully data-driven. Adding a new rule is usually a two-line TOML diff plus a fixture. See [Writing a rule](https://a-hackers-guide.github.io/crossfire/writing-rules/) for the full schema and conventions.

Narrow regexes beat broad ones. Prefer `warn` over `block` unless the code genuinely cannot build on the target.

## Commit style

- **Subject line** ≤ 72 chars, imperative mood: `feat(engine): add macOS kqueue rule group`.
- **Body** wrapped at 72 chars. Explain *why* if the subject doesn't already say it.
- **Signed-off / Co-authored-by** lines are welcome but not required.

## CHANGELOG

Behaviour-visible changes get a one-liner in `CHANGELOG.md` under the `[Unreleased]` section:

```md
## [Unreleased]

### Added

- `win.input.rawinput` rule group (RAWINPUT API family).
```

Pure refactors / internal clean-ups don't need a CHANGELOG entry.

## Code of conduct

Be kind, assume good faith, and don't break things on purpose. Harassment, spam, or targeting individuals will get you banned from the repo. If you see something, [open a private security advisory](https://github.com/A-Hackers-Guide/crossfire/security/advisories/new) — same channel we use for security reports — and we'll handle it quietly.

## Licence

All contributions are MIT-licenced. By opening a PR you agree your changes can be distributed under that licence.
