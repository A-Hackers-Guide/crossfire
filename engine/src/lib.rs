//! Crossfire conversion engine.
//!
//! Scans a source tree for OS-specific code, asks tailored questions, converts
//! the project for Windows / macOS / Linux / FreeBSD (rule-based rewrites,
//! portability shims, build/CI/packaging templates and an optional AI pass),
//! detects local toolchains, builds per target and smoke-tests Windows builds
//! through Game Porting Toolkit.
//!
//! All long-running functions are blocking and report progress through the
//! `on` callback; call them from a worker thread in async hosts.

pub mod ai;
pub mod build;
pub mod convert;
pub mod detect;
pub mod gptk;
pub mod lang;
pub mod model;
pub mod questions;
pub mod rewrite;
pub mod rules;
pub mod scan;
pub mod shim;
pub mod stubs;
pub mod templates;
pub mod toolchain;
pub mod util;

pub use model::*;

use anyhow::Result;
use std::path::Path;

/// Scan a project directory.
pub fn scan(root: &Path, on: &dyn Fn(ProgressEvent)) -> Result<ScanReport> {
    scan::scan(root, on)
}

/// Questions tailored to a scan and the chosen target OSes.
pub fn questions(scan: &ScanReport, targets: &[Os]) -> Vec<Question> {
    questions::questions(scan, targets)
}

/// Convert `req.source` into `req.output` (the source is never modified).
pub fn convert(req: &ConvertRequest, on: &dyn Fn(ProgressEvent)) -> Result<ConvertResult> {
    convert::convert(req, on)
}

/// Detect compilers, cross-compilers, build tools and runtimes on this host.
pub fn toolchains() -> Vec<Toolchain> {
    toolchain::toolchains()
}

/// Build a project for each target (locally where possible, else via CI).
pub fn build(req: &BuildRequest, on: &dyn Fn(ProgressEvent)) -> Result<BuildResult> {
    build::build(req, on)
}

/// Run a Windows executable through Game Porting Toolkit (macOS) or Wine.
pub fn test_with_gptk(exe: &Path, on: &dyn Fn(ProgressEvent)) -> Result<String> {
    gptk::test_with_gptk(exe, on)
}

/// The OS this engine runs on.
pub fn host_os() -> Os {
    Os::host()
}

/// Number of rules in the embedded rule tables.
pub fn rule_count() -> usize {
    rules::rules().len()
}
