# Security policy

## Reporting a vulnerability

Please **do not** open a public issue for security problems.

- **Preferred:** GitHub's [private security advisory](https://github.com/A-Hackers-Guide/crossfire/security/advisories/new) workflow.
- **Email fallback:** open a draft PR titled `SECURITY` with contact details and we'll move it off the public surface.

We'll acknowledge within 72 hours and, for anything that affects users, aim to ship a fix within 14 days.

## What this project's threat model is

Crossfire runs trusted code (your compilers, your build tools) against untrusted input (a source tree you point it at). The engine:

- **Reads** the input tree. It does not write inside the input.
- **Writes** a new output tree. The output and any intermediate files land in paths you supply.
- **Runs** compilers, package tools, and — if you opt in — an AI endpoint. It does not sandbox any of them.
- **Does not** connect to the network except through the explicit AI pass (and only to the endpoint you configured).
- **Does not** read `/etc/shadow`, your SSH keys, or anything outside the input and output trees.

That means:

- **In scope** — anything that lets a malicious input tree escape those boundaries (e.g. a crafted filename makes the engine write outside the output tree, a zip-slip, a path-traversal in a template, an injection into a `std::process::Command`, an integer-overflow panic that denial-of-services the GUI).
- **In scope** — anything that leaks `CROSSFIRE_AI_KEY`, writes it to the project file, or sends it to a host other than the configured endpoint.
- **In scope** — supply-chain regressions (yanked crates, licence conflicts that reach a release, new `unsafe` blocks that weren't reviewed).
- **Out of scope** — the fact that your build system can run arbitrary code. Crossfire is not a build-system sandbox. If you convert a project whose `CMakeLists.txt` runs `rm -rf /`, that still runs.

## Supply chain

Every PR goes through `cargo-deny check` for advisories, bans, licences, and sources. The allow-list lives in [`engine/deny.toml`](engine/deny.toml) and only accepts the usual OSS licences (MIT / Apache-2.0 / BSD / ISC / MPL-2.0 / BSL / CC0 / Unicode / Zlib / CDLA-Permissive-2.0). Unknown registries and git-sourced crates are denied.

See the [Lint workflow](.github/workflows/lint.yml) for the exact gates.

## Signed releases

Releases are built in CI on hosted runners from a tagged commit. Each artefact is attached to a GitHub Release. If you need reproducible builds, build from source — the CI workflow is reproducible by design, and we have the exact toolchain pinned per OS.

Future work: cosign-signed binaries ([tracked internally](#)).

## Thanks

We credit reporters in the changelog once a fix ships, unless you ask us not to.
