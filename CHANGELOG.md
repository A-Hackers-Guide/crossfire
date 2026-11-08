# Changelog

All notable changes to Crossfire. Dates are UTC; versions follow [SemVer](https://semver.org/).

## [Unreleased]

### Added

- **CI linting workflow** (`.github/workflows/lint.yml`): five jobs that fail PRs on style / lint / supply-chain regressions:
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo-deny check` (advisories + bans + licenses + sources) with policy in `engine/deny.toml`
  - `oxlint` on the UI
  - `tsc -b --noEmit` on the UI
- **GitHub Pages docs site** at <https://a-hackers-guide.github.io/crossfire/> built with `mkdocs-material` 9.5.44 strict mode. 13 pages:
  - `index`, `install`, `first-port`
  - `cli` (crossfire-engine reference)
  - `rules`, `writing-rules`
  - `target-matrix`, `shim`, `build`, `toolchains`
  - `ai`, `architecture`, `faq`
- **Docs workflow** (`.github/workflows/docs.yml`) that rebuilds + redeploys on every push to `main` touching `docs/`, `README.md`, or `mkdocs.yml`.
- **README promises list** — 50 numbered claims, each backed by code or CI landed in this session.
- **`CHANGELOG.md`** (this file).

### Changed

- **Applied `cargo fmt` across the entire engine tree.** The rustfmt gate now goes green on its first run.
- **`engine/deny.toml`** allows the OSS licenses this repo's transitive tree actually uses: MIT, Apache-2.0 (including LLVM exception), BSD-2/3, ISC, MPL-2.0, BSL-1.0, CC0-1.0, Unicode-DFS-2016, Unicode-3.0, Zlib, CDLA-Permissive-2.0 (webpki-roots). Unknown registries and git sources are denied.
