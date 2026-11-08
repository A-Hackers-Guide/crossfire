<!-- Thanks for sending a PR to Crossfire. Keep this template light; drop
     sections that don't apply. -->

## What this changes



## Why



## Scope

- [ ] Engine (Rust) code
- [ ] UI (React / TypeScript) code
- [ ] Rule tables (`engine/src/rules/*.toml`)
- [ ] Generated templates (`templates.rs`, CMake / CI / packaging)
- [ ] AI pass (`ai.rs`)
- [ ] Docs (`docs/**`, `README.md`)
- [ ] CI (`.github/workflows/**`)

## Checklist

- [ ] `cargo fmt` is clean
- [ ] `cargo clippy --all-targets -- -D warnings` passes
- [ ] `cargo test` passes on at least one host
- [ ] If this adds a rule, a fixture was added and matched
- [ ] If this touches the engine ↔ UI contract, the shape change is
      documented in the PR body or in `docs/architecture.md`
- [ ] If this is a behavior change, `CHANGELOG.md` has a line under
      `[Unreleased]`

## Follow-ups worth filing as separate issues
