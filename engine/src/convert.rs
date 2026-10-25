//! Conversion pipeline: copy → rule rewrites → optional AI pass → rescan →
//! shims, stubs, build/CI/packaging templates → diffs + report.
//!
//! The source tree is never modified; everything is written to `req.output`.

use crate::ai;
use crate::lang;
use crate::model::*;
use crate::questions::effective_answers;
use crate::rewrite::{self, RewriteCtx};
use crate::scan::{rel_string, scan_full, FileInfo};
use crate::shim;
use crate::stubs;
use crate::templates::{self, AffineSource, BuildKind, CiPlan, NativePlan};
use anyhow::{bail, Context, Result};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Instant;

/// Directories not copied into the output (build products, caches, VCS).
const COPY_SKIP: &[&str] = &[
    ".git", ".hg", ".svn", "node_modules", "target", "build", "bin", "obj", "DerivedData", ".vs",
    ".idea", "__pycache__", ".venv", "venv", "Library", "Temp", "Logs", ".godot", "Binaries",
    "Intermediate", "Saved", "zig-cache", ".zig-cache", "zig-out", ".gradle", ".dart_tool",
];

/// Per-file state tracked through the pipeline.
struct Tracked {
    original: Option<String>,
    method: ChangeMethod,
    confidence: f64,
    notes: Vec<String>,
}

struct Output<'a> {
    root: &'a Path,
    changes: BTreeMap<String, Tracked>,
}

impl Output<'_> {
    fn path(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    /// Write a generated or rewritten file and record the change.
    fn write(&mut self, rel: &str, content: &str, method: ChangeMethod, confidence: f64, notes: Vec<String>) -> Result<()> {
        let p = self.path(rel);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let entry = self.changes.entry(rel.to_string()).or_insert_with(|| Tracked {
            original: None,
            method,
            confidence,
            notes: vec![],
        });
        if entry.original.is_none() && p.exists() {
            entry.original = std::fs::read_to_string(&p).ok();
        }
        entry.method = method;
        entry.confidence = confidence;
        for n in notes {
            if !entry.notes.contains(&n) {
                entry.notes.push(n);
            }
        }
        std::fs::write(&p, content).with_context(|| format!("writing {}", p.display()))
    }

    fn set_executable(&self, rel: &str) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(self.path(rel), std::fs::Permissions::from_mode(0o755));
        }
        #[cfg(not(unix))]
        let _ = rel;
    }
}

fn is_unit(id: &str) -> bool {
    matches!(id, "c" | "cpp" | "objc" | "objcpp")
}

fn sanitize_name(s: &str) -> String {
    let mut n: String = s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' }).collect();
    n = n.trim_matches('_').to_string();
    if n.is_empty() || n.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
        n = format!("app_{n}");
    }
    n
}

/// Copy the project into `out`, skipping VCS/build dirs and `exclude`.
fn copy_tree(src: &Path, out: &Path, exclude: &Path) -> Result<Vec<String>> {
    let exclude = exclude.to_path_buf();
    let walker = ignore::WalkBuilder::new(src)
        .hidden(false)
        .git_ignore(true)
        .git_exclude(true)
        .git_global(false)
        .require_git(false)
        .parents(false)
        .filter_entry(move |e| {
            if e.depth() == 0 {
                return true;
            }
            if e.path().starts_with(&exclude) {
                return false;
            }
            let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
            let name = e.file_name().to_string_lossy();
            !(is_dir && (COPY_SKIP.contains(&name.as_ref()) || name.starts_with("cmake-build-")))
        })
        .build();
    let mut copied = Vec::new();
    for e in walker.flatten() {
        if !e.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let Ok(rel) = e.path().strip_prefix(src) else { continue };
        let dst = out.join(rel);
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(e.path(), &dst).with_context(|| format!("copying {}", e.path().display()))?;
        copied.push(rel_string(rel));
    }
    copied.sort();
    Ok(copied)
}

fn progress(on: &dyn Fn(ProgressEvent), p: f64, msg: impl Into<String>) {
    on(ProgressEvent::new(Stage::Convert, Level::Info, p, msg));
}

/// Public conversion entry point.
pub fn convert(req: &ConvertRequest, on: &dyn Fn(ProgressEvent)) -> Result<ConvertResult> {
    let started = Instant::now();
    let source = PathBuf::from(&req.source);
    if !source.is_dir() {
        bail!("source {} is not a directory", source.display());
    }
    let source = source.canonicalize()?;
    std::fs::create_dir_all(&req.output).with_context(|| format!("creating {}", req.output))?;
    let output = PathBuf::from(&req.output).canonicalize()?;
    if output == source || source.starts_with(&output) {
        bail!("output must not be the source directory or one of its parents");
    }

    // 1. Scan the source --------------------------------------------------------------
    progress(on, 0.0, format!("Scanning {}", source.display()));
    let src_scan = scan_full(&source, &|e| {
        let p = if e.progress >= 0.0 { e.progress * 0.15 } else { -1.0 };
        on(ProgressEvent { stage: Stage::Convert, progress: p, ..e });
    })?;
    let targets: Vec<Os> = if req.targets.is_empty() { Os::ALL.to_vec() } else { req.targets.clone() };
    let source_os: Vec<Os> = if req.source_os.is_empty() { src_scan.report.detected_os.clone() } else { req.source_os.clone() };
    let answers = effective_answers(&src_scan.report, &targets, &req.answers);
    let strategy = answers.str_or("strategy", "sdl3");
    let idiomatic = answers.str_or("style", "strict") == "idiomatic";
    let keep_originals = answers.bool_or("keepOriginals", false);

    // 2. Copy ----------------------------------------------------------------------------
    progress(on, 0.15, format!("Copying project to {}", output.display()));
    let copied = copy_tree(&source, &output, &output)?;
    let mut out = Output { root: &output, changes: BTreeMap::new() };

    // 3. Rule-based rewrites ------------------------------------------------------------------
    let files_lower: HashMap<String, String> = copied.iter().map(|r| (r.to_ascii_lowercase(), r.clone())).collect();
    let mut include_dirs: BTreeSet<String> = BTreeSet::new();
    for r in &copied {
        if matches!(Path::new(r).extension().and_then(|e| e.to_str()), Some("h" | "hpp" | "hh" | "hxx" | "inl")) {
            include_dirs.insert(r.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_default());
        }
    }
    let include_dirs: Vec<String> = include_dirs.into_iter().filter(|d| !d.is_empty()).collect();
    let cx = RewriteCtx {
        targets: &targets,
        idiomatic,
        fix_include_case: answers.bool_or("fixIncludeCase", true),
        files_lower: &files_lower,
        include_dirs: &include_dirs,
    };
    let info_by_rel: HashMap<&str, &FileInfo> = src_scan.files.iter().map(|f| (f.rel.as_str(), f)).collect();
    let mut uses = Uses::default();
    let total = copied.len().max(1);
    for (i, rel) in copied.iter().enumerate() {
        let Some(info) = info_by_rel.get(rel.as_str()) else { continue };
        let Some(l) = info.lang else { continue };
        if !info.text {
            continue;
        }
        let path = out.path(rel);
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        let res = match l.id {
            id if lang::is_c_family(id) => {
                uses.c = true;
                rewrite::rewrite_c(rel, id, &text, &cx)
            }
            "python" => {
                uses.python = true;
                rewrite::rewrite_python(rel, &text)
            }
            "csharp" => {
                uses.csharp = true;
                rewrite::rewrite_csharp(&text)
            }
            "msbuild" if rel.ends_with(".csproj") => rewrite::rewrite_csproj(&text),
            "shell" if targets.contains(&Os::Freebsd) => rewrite::rewrite_shell(&text),
            _ => None,
        };
        if let Some(r) = res {
            if keep_originals {
                let _ = std::fs::write(out.path(&format!("{rel}.orig")), &text);
            }
            out.write(rel, &r.text, ChangeMethod::Rule, 0.95, r.notes)?;
            if (i + 1) % 10 == 0 || i + 1 == total {
                on(ProgressEvent::new(Stage::Convert, Level::Info, 0.2 + 0.25 * (i + 1) as f64 / total as f64, "Applied rule-based rewrites").with_file(rel.clone()));
            }
        }
    }
    uses.python |= src_scan.files.iter().any(|f| f.lang.map(|l| l.id == "python").unwrap_or(false));
    uses.csharp |= src_scan.files.iter().any(|f| f.lang.map(|l| l.id == "csharp").unwrap_or(false));
    uses.c |= src_scan.files.iter().any(|f| f.lang.map(|l| lang::is_c_family(l.id)).unwrap_or(false));
    progress(on, 0.45, format!("Rule pass changed {} file(s)", out.changes.len()));

    // 4. AI pass for hard files --------------------------------------------------------------------
    let mut ai_notes: Vec<String> = Vec::new();
    if answers.bool_or("useAi", true) && ai::available(&req.ai) {
        run_ai_pass(&mut out, &src_scan.report, &src_scan.files, &targets, &source_os, &answers, &req.ai, on, &mut ai_notes)?;
    } else if answers.bool_or("useAi", false) && !ai::available(&req.ai) {
        ai_notes.push("AI pass requested but no API key/provider is configured; skipped.".into());
    }

    // 5. Rescan the output to see what remains ------------------------------------------------------
    progress(on, 0.8, "Re-scanning converted tree");
    let post = scan_full(&output, &|_| {})?;

    // 6. Shims, stubs and templates -----------------------------------------------------------------------
    progress(on, 0.85, "Generating shims, build files, CI and packaging");
    let kind = build_kind(&src_scan.report, &post.files, &answers);
    let mut plan_info = PlanInfo::default();
    if uses.c {
        out.write("crossfire/cf_platform.h", shim::CF_PLATFORM_H, ChangeMethod::Template, 1.0, vec!["Portability layer: helpers + Win32/POSIX compatibility subset".into()])?;
        out.write("crossfire/cf_platform.c", shim::CF_PLATFORM_C, ChangeMethod::Template, 1.0, vec!["Implementation of cf_platform.h for Windows, macOS, Linux and FreeBSD".into()])?;
        plan_info = native_plan(&mut out, &src_scan.report, &post.files, &post.report.findings, &targets, &answers, &strategy)?;
        generate_native_build(&mut out, &plan_info.plan, &answers, &src_scan.report)?;
    }
    if uses.python {
        out.write("crossfire/__init__.py", shim::CF_INIT_PY, ChangeMethod::Template, 1.0, vec![])?;
        out.write("crossfire/cf_platform.py", shim::CF_PLATFORM_PY, ChangeMethod::Template, 1.0, vec!["Portable replacements for os.startfile, %APPDATA%, cls, msvcrt.getch".into()])?;
        out.write("crossfire/cf_winreg.py", shim::CF_WINREG_PY, ChangeMethod::Template, 1.0, vec!["winreg subset backed by JSON outside Windows".into()])?;
        out.write("crossfire/cf_msvcrt.py", shim::CF_MSVCRT_PY, ChangeMethod::Template, 1.0, vec!["msvcrt subset (getch/kbhit) for POSIX".into()])?;
    }
    if uses.csharp {
        let dir = src_scan
            .files
            .iter()
            .filter(|f| f.rel.ends_with(".csproj"))
            .min_by_key(|f| f.rel.matches('/').count())
            .and_then(|f| f.rel.rsplit_once('/').map(|(d, _)| format!("{d}/")))
            .unwrap_or_default();
        out.write(&format!("{dir}Crossfire/CfPlatform.cs"), shim::CF_PLATFORM_CS, ChangeMethod::Template, 1.0, vec!["CfPlatform (paths, OpenPath) and CfSettings (registry replacement)".into()])?;
    }
    let display = src_scan.report.name.clone();
    let name = sanitize_name(&display);
    let packages = answers.list("packages");
    let mut ci_path = None;
    if answers.bool_or("ci", true) {
        let mut apt = BTreeSet::new();
        let mut pkg = BTreeSet::new();
        for (os, ls) in &plan_info.plan.links {
            match os {
                Os::Linux => apt.extend(ls.packages.iter().cloned()),
                Os::Freebsd => pkg.extend(ls.packages.iter().cloned()),
                _ => {}
            }
        }
        let wf = templates::workflow(&CiPlan {
            name: &name,
            kind: &kind,
            targets: &targets,
            apt: apt.into_iter().collect(),
            pkg: pkg.into_iter().filter(|p| !p.is_empty()).collect(),
            sdl3: uses.c && plan_info.plan.sdl3_default,
            packages: &packages,
        });
        let p = ".github/workflows/crossfire-build.yml";
        out.write(p, &wf, ChangeMethod::Template, 1.0, vec!["GitHub Actions: windows/macos/ubuntu runners + FreeBSD via vmactions/freebsd-vm".into()])?;
        ci_path = Some(p.to_string());
    }
    generate_packaging(&mut out, &name, &display, &answers, &packages, &src_scan.report)?;

    // 7. Changes, diffs, report --------------------------------------------------------------------------
    progress(on, 0.95, "Writing diffs and report");
    let ignore_review = |f: &Finding| {
        f.file.starts_with("crossfire/") || f.file.starts_with("packaging/") || f.file.starts_with(".github/") || f.file.ends_with(".orig") || f.file.contains("/Crossfire/CfPlatform.cs")
    };
    let manual_review: Vec<Finding> = post
        .report
        .findings
        .iter()
        .filter(|f| f.severity >= Severity::Medium && !ignore_review(f))
        .cloned()
        .collect();
    let mut changes = Vec::new();
    for (rel, t) in &out.changes {
        let new = std::fs::read_to_string(out.path(rel)).unwrap_or_default();
        let action = if t.original.is_some() || copied.contains(rel) { FileAction::Modified } else { FileAction::Added };
        let old = t.original.clone().unwrap_or_default();
        let diff = if new.len() + old.len() < 400_000 {
            Some(
                similar::TextDiff::from_lines(&old, &new)
                    .unified_diff()
                    .context_radius(3)
                    .header(&format!("a/{rel}"), &format!("b/{rel}"))
                    .to_string(),
            )
        } else {
            None
        };
        changes.push(FileChange { path: rel.clone(), action, method: t.method, confidence: t.confidence, notes: t.notes.clone(), diff });
    }
    for rel in copied.iter().filter(|r| !out.changes.contains_key(*r)) {
        changes.push(FileChange { path: rel.clone(), action: FileAction::Copied, method: ChangeMethod::Copy, confidence: 1.0, notes: vec![], diff: None });
    }
    changes.sort_by(|a, b| {
        let rank = |c: &FileChange| match c.action {
            FileAction::Modified => 0,
            FileAction::Added => 1,
            FileAction::Removed => 2,
            FileAction::Copied => 3,
        };
        rank(a).cmp(&rank(b)).then(a.path.cmp(&b.path))
    });

    let report = report_markdown(&ReportCtx {
        src: &src_scan.report,
        post: &post.report,
        targets: &targets,
        source_os: &source_os,
        answers: &answers,
        changes: &changes,
        manual: &manual_review,
        plan: &plan_info,
        kind: &kind,
        ai_notes: &ai_notes,
        ci: ci_path.as_deref(),
    });
    let report_rel = "CROSSFIRE_REPORT.md";
    std::fs::write(out.path(report_rel), &report)?;
    changes.insert(
        changes.iter().position(|c| c.action != FileAction::Modified).unwrap_or(changes.len()),
        FileChange { path: report_rel.into(), action: FileAction::Added, method: ChangeMethod::Template, confidence: 1.0, notes: vec!["Conversion summary, manual review list and next steps".into()], diff: None },
    );

    let modified = changes.iter().filter(|c| c.action == FileAction::Modified).count();
    let added = changes.iter().filter(|c| c.action == FileAction::Added).count();
    on(ProgressEvent::new(
        Stage::Convert,
        Level::Success,
        1.0,
        format!("Converted: {modified} modified, {added} added, {} need manual review (portability {:.0}% → {:.0}%)", manual_review.len(), src_scan.report.portability, post.report.portability),
    ));
    Ok(ConvertResult {
        output: output.to_string_lossy().to_string(),
        changes,
        manual_review,
        report_path: output.join(report_rel).to_string_lossy().to_string(),
        duration_ms: started.elapsed().as_millis() as u64,
    })
}

#[derive(Default)]
struct Uses {
    c: bool,
    python: bool,
    csharp: bool,
}

// ---------------------------------------------------------------------------
// AI pass
// ---------------------------------------------------------------------------

/// (path, language, content, findings)
type AiJob = (String, String, String, Vec<Finding>);

#[allow(clippy::too_many_arguments)]
fn run_ai_pass(
    out: &mut Output,
    report: &ScanReport,
    files: &[FileInfo],
    targets: &[Os],
    source_os: &[Os],
    answers: &Answers,
    settings: &AiSettings,
    on: &dyn Fn(ProgressEvent),
    notes: &mut Vec<String>,
) -> Result<()> {
    let mut hard: BTreeMap<&str, Vec<Finding>> = BTreeMap::new();
    for f in report.findings.iter().filter(|f| f.severity >= Severity::High) {
        hard.entry(f.file.as_str()).or_default().push(f.clone());
    }
    // Skip huge files: they would not round-trip within max_tokens.
    let jobs: Vec<AiJob> = hard
        .into_iter()
        .filter_map(|(rel, fs)| {
            let info = files.iter().find(|f| f.rel == rel)?;
            let lang = info.lang?.name.to_string();
            let content = std::fs::read_to_string(out.path(rel)).ok()?;
            if content.len() > 120_000 {
                notes.push(format!("{rel}: too large for one AI pass ({} KB); left for manual review", content.len() / 1024));
                return None;
            }
            Some((rel.to_string(), lang, content, fs))
        })
        .collect();
    if jobs.is_empty() {
        return Ok(());
    }
    let n = jobs.len();
    progress(on, 0.5, format!("AI pass: rewriting {n} file(s) with {}", if settings.model.is_empty() { ai::DEFAULT_ANTHROPIC_MODEL } else { &settings.model }));
    // Up to 4 concurrent requests; results flow back over a channel so the
    // progress callback stays on this thread. HTTP runs on plain threads,
    // never inside an async runtime.
    let (tx, rx) = mpsc::channel::<(String, Result<String>)>();
    let mut results = Vec::new();
    std::thread::scope(|s| {
        let chunks: Vec<Vec<&AiJob>> = {
            let mut c: Vec<Vec<_>> = (0..4.min(n)).map(|_| Vec::new()).collect();
            for (i, j) in jobs.iter().enumerate() {
                let k = i % c.len();
                c[k].push(j);
            }
            c
        };
        for chunk in chunks {
            let tx = tx.clone();
            s.spawn(move || {
                for (rel, lang, content, fs) in chunk {
                    let r = ai::rewrite(
                        settings,
                        &ai::AiRequest { path: rel, language: lang, content, findings: fs, targets, source_os, answers },
                    );
                    let _ = tx.send((rel.clone(), r));
                }
            });
        }
        drop(tx);
        for (done, (rel, r)) in rx.iter().enumerate() {
            let level = if r.is_ok() { Level::Info } else { Level::Warn };
            let msg = match &r {
                Ok(_) => format!("AI rewrote {rel}"),
                Err(e) => format!("AI failed for {rel}: {e:#} — keeping rule-based version"),
            };
            on(ProgressEvent::new(Stage::Convert, level, 0.5 + 0.3 * (done + 1) as f64 / n as f64, msg).with_file(rel.clone()));
            results.push((rel, r));
        }
    });
    for (rel, r) in results {
        match r {
            Ok(text) => {
                let original_len = std::fs::read_to_string(out.path(&rel)).map(|s| s.len()).unwrap_or(0);
                let mut conf = 0.7;
                if original_len > 0 && (text.len() as f64) < original_len as f64 * 0.4 {
                    conf = 0.45; // suspiciously short rewrite
                }
                out.write(&rel, &text, ChangeMethod::Ai, conf, vec![format!("Rewritten by AI ({}) for {}", if settings.model.is_empty() { ai::DEFAULT_ANTHROPIC_MODEL } else { &settings.model }, targets.iter().map(|o| o.label()).collect::<Vec<_>>().join(", "))])?;
            }
            Err(e) => {
                let msg = format!("AI pass failed: {e:#}; rule-based version kept");
                if let Some(t) = out.changes.get_mut(&rel) {
                    t.notes.push(msg.clone());
                } else {
                    notes.push(format!("{rel}: {msg}"));
                }
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Native (C/C++/Objective-C) build plan
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct PlanInfo {
    pub plan: NativePlan,
    /// (source, OS label, generated stub path, functions stubbed)
    pub stubs: Vec<(String, String, String, Vec<String>)>,
}

impl Default for NativePlan {
    fn default() -> Self {
        NativePlan {
            name: String::new(),
            display_name: String::new(),
            bundle_id: String::new(),
            common_sources: vec![],
            affine: vec![],
            include_dirs: vec![],
            executable: false,
            win32_subsystem: false,
            gui: false,
            has_objc: false,
            cxx_standard: "20".into(),
            sdl3_default: false,
            macos_arch: "universal".into(),
            links: vec![],
            packages: vec![],
            excluded: vec![],
        }
    }
}

fn os_list_label(v: &[Os]) -> String {
    v.iter().map(|o| o.label()).collect::<Vec<_>>().join("/")
}

/// The OS set a translation unit is tied to, if any.
fn affinity(info: &FileInfo, findings: &[&Finding]) -> Option<Vec<Os>> {
    let id = info.lang?.id;
    let mut set: Option<Vec<Os>> = if matches!(id, "objc" | "objcpp") { Some(vec![Os::Macos]) } else { None };
    let strong: Vec<&&Finding> = findings.iter().filter(|f| f.severity >= Severity::High && f.os.len() < 4).collect();
    let blockers = strong.iter().filter(|f| f.severity == Severity::Blocker).count();
    if set.is_none() && blockers == 0 && strong.len() < 2 {
        return None;
    }
    for f in strong {
        set = Some(match set {
            None => f.os.clone(),
            Some(s) => s.into_iter().filter(|o| f.os.contains(o)).collect(),
        });
    }
    match set {
        Some(s) if !s.is_empty() && s.len() < 4 => Some(s),
        _ => None,
    }
}

fn native_plan(
    out: &mut Output,
    report: &ScanReport,
    files: &[FileInfo],
    findings: &[Finding],
    targets: &[Os],
    answers: &Answers,
    strategy: &str,
) -> Result<PlanInfo> {
    let name = sanitize_name(&report.name);
    let mut by_file: HashMap<&str, Vec<&Finding>> = HashMap::new();
    for f in findings {
        by_file.entry(f.file.as_str()).or_default().push(f);
    }
    let units: Vec<&FileInfo> = files
        .iter()
        .filter(|f| f.lang.map(|l| is_unit(l.id)).unwrap_or(false) && !f.rel.starts_with("crossfire/") && !f.rel.ends_with(".orig"))
        .collect();
    let translation = strategy == "translation";
    let aff: HashMap<&str, Vec<Os>> = if translation {
        HashMap::new()
    } else {
        units
            .iter()
            .filter_map(|f| affinity(f, by_file.get(f.rel.as_str()).map(|v| v.as_slice()).unwrap_or(&[])).map(|a| (f.rel.as_str(), a)))
            .collect()
    };

    // Entry points: one portable main wins; otherwise per-OS entries.
    let entries: Vec<&FileInfo> = units.iter().copied().filter(|f| f.has_entry).collect();
    let score = |f: &FileInfo| {
        let stem = Path::new(&f.rel).file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
        (f.rel.matches('/').count(), if stem == "main" { 0 } else { 1 }, f.rel.clone())
    };
    let portable_entries: Vec<&FileInfo> = entries.iter().copied().filter(|f| !aff.contains_key(f.rel.as_str())).collect();
    let mut excluded = Vec::new();
    let mut primary: Option<&FileInfo> = None;
    if !portable_entries.is_empty() {
        let mut pe = portable_entries.clone();
        pe.sort_by_key(|f| score(f));
        primary = Some(pe[0]);
        excluded.extend(pe[1..].iter().map(|f| f.rel.clone()));
        excluded.extend(entries.iter().filter(|f| aff.contains_key(f.rel.as_str())).map(|f| f.rel.clone()));
    }

    let mut plan = NativePlan {
        name: name.clone(),
        display_name: report.name.clone(),
        bundle_id: answers.str_or("bundleId", &format!("com.example.{}", name.to_lowercase())),
        include_dirs: {
            let mut d: BTreeSet<String> = BTreeSet::new();
            for f in files.iter().filter(|f| f.lang.map(|l| l.id == "h").unwrap_or(false) || f.rel.ends_with(".hpp")) {
                if let Some((dir, _)) = f.rel.rsplit_once('/') {
                    if !dir.starts_with("crossfire") {
                        d.insert(dir.to_string());
                    }
                }
            }
            d.into_iter().collect()
        },
        executable: !entries.is_empty(),
        gui: report.kinds.iter().any(|k| k.flavor == "game" || k.flavor == "app"),
        has_objc: units.iter().any(|f| matches!(f.lang.map(|l| l.id), Some("objc" | "objcpp"))),
        cxx_standard: answers.str_or("cppStandard", "20"),
        sdl3_default: matches!(strategy, "sdl3" | "hybrid")
            || ["windowing", "audio", "input"].iter().any(|k| answers.str_or(k, "") == "sdl3"),
        macos_arch: answers.str_or("macosArch", "universal"),
        packages: answers.list("packages"),
        excluded: excluded.clone(),
        ..Default::default()
    };
    let win32_re = regex::Regex::new(r"\b(WinMain|wWinMain|_tWinMain)\s*\(").unwrap();
    if let Some(p) = primary {
        let text = std::fs::read_to_string(out.path(&p.rel)).unwrap_or_default();
        plan.win32_subsystem = win32_re.is_match(&text);
    }

    let mut info = PlanInfo::default();
    let mut affine_entries: Vec<&FileInfo> = Vec::new();
    for u in &units {
        if excluded.contains(&u.rel) {
            continue;
        }
        match aff.get(u.rel.as_str()) {
            None => plan.common_sources.push(u.rel.clone()),
            Some(oses) if u.has_entry => {
                affine_entries.push(u);
                plan.affine.push(AffineSource { path: u.rel.clone(), oses: oses.clone(), stub: None });
            }
            Some(oses) => {
                let stub_rel = write_stub(out, u, oses, &mut info)?;
                plan.affine.push(AffineSource { path: u.rel.clone(), oses: oses.clone(), stub: stub_rel });
            }
        }
    }
    // Per-OS entry points only: add a stub main for OSes none of them cover.
    if primary.is_none() && !affine_entries.is_empty() {
        let covered: BTreeSet<Os> = affine_entries.iter().flat_map(|e| aff[e.rel.as_str()].clone()).collect();
        let missing: Vec<Os> = Os::ALL.iter().copied().filter(|o| !covered.contains(o)).collect();
        if !missing.is_empty() {
            let e = affine_entries[0];
            let oses = aff[e.rel.as_str()].clone();
            if let Some(stub_rel) = write_stub(out, e, &oses, &mut info)? {
                plan.affine.push(AffineSource { path: stub_rel, oses: missing, stub: None });
            }
        }
        // A Windows-only WinMain entry needs the GUI subsystem on Windows.
        plan.win32_subsystem = affine_entries.iter().any(|e| aff[e.rel.as_str()] == vec![Os::Windows]);
    }

    // Link requirements per target OS from the APIs used by sources compiled there.
    for os in targets.iter().copied().chain(Os::ALL) {
        if plan.links.iter().any(|(o, _)| *o == os) {
            continue;
        }
        let compiled: BTreeSet<&str> = plan
            .common_sources
            .iter()
            .map(String::as_str)
            .chain(plan.affine.iter().filter(|a| a.oses.contains(&os)).map(|a| a.path.as_str()))
            .collect();
        let mut apis: BTreeSet<String> = BTreeSet::new();
        for f in findings {
            let header_used = !lang::classify(Path::new(&f.file)).map(|l| is_unit(l.id)).unwrap_or(true);
            if (compiled.contains(f.file.as_str()) || header_used) && (f.os.contains(&os) || f.os.len() == 4) {
                apis.insert(f.api.clone());
            }
        }
        plan.links.push((os, templates::link_set(os, &apis)));
    }
    info.plan = plan;
    Ok(info)
}

/// Generate the stub for an OS-affine unit; returns its path.
fn write_stub(out: &mut Output, u: &FileInfo, oses: &[Os], info: &mut PlanInfo) -> Result<Option<String>> {
    let text = std::fs::read_to_string(out.path(&u.rel)).unwrap_or_default();
    let cpp = matches!(u.lang.map(|l| l.id), Some("cpp" | "objcpp"));
    let dir = u.rel.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
    // Re-point local includes at project-root-relative paths that exist.
    let inc_re = regex::Regex::new(r#"(?m)^\s*#\s*include\s*"([^"]+)""#).unwrap();
    let mut includes = Vec::new();
    for c in inc_re.captures_iter(&text) {
        let inc = &c[1];
        if inc.contains("cf_platform.h") {
            continue;
        }
        let joined = if dir.is_empty() { inc.to_string() } else { format!("{dir}/{inc}") };
        let normalized = normalize(&joined);
        if out.path(&normalized).is_file() {
            includes.push(normalized);
        } else if out.path(inc).is_file() {
            includes.push(inc.to_string());
        }
    }
    let label = os_list_label(oses);
    let (stub, names) = stubs::generate_stub(&u.rel, &text, cpp, &includes, &label);
    if names.is_empty() {
        return Ok(None);
    }
    let ext = match u.lang.map(|l| l.id) {
        Some("cpp" | "objcpp") => "cpp",
        _ => "c",
    };
    let stub_rel = format!("crossfire/stubs/{}.stub.{ext}", u.rel.replace('/', "_").rsplit_once('.').map(|(a, _)| a).unwrap_or(&u.rel));
    out.write(&stub_rel, &stub, ChangeMethod::Template, 0.5, vec![format!("Stub for {} on non-{label} targets ({} functions)", u.rel, names.len())])?;
    info.stubs.push((u.rel.clone(), label, stub_rel.clone(), names));
    Ok(Some(stub_rel))
}

fn normalize(p: &str) -> String {
    let mut v: Vec<&str> = Vec::new();
    for s in p.split('/') {
        match s {
            "" | "." => {}
            ".." => {
                v.pop();
            }
            x => v.push(x),
        }
    }
    v.join("/")
}

fn generate_native_build(out: &mut Output, plan: &NativePlan, answers: &Answers, report: &ScanReport) -> Result<()> {
    let has_cmake = report.kinds.iter().any(|k| k.name == "CMake");
    let choice = answers.str_or("buildSystem", if has_cmake { "keep" } else { "cmake" });
    let cmake_root_exists = out.path("CMakeLists.txt").is_file();
    match choice.as_str() {
        "keep" if cmake_root_exists => {
            out.write("crossfire/crossfire.cmake", &templates::crossfire_cmake(plan), ChangeMethod::Template, 0.9, vec!["Per-OS sources/libraries for an existing CMake project".into()])?;
            // Hook it into the existing CMakeLists.txt after the first target.
            let txt = std::fs::read_to_string(out.path("CMakeLists.txt")).unwrap_or_default();
            let re = regex::Regex::new(r"(?m)^\s*add_(executable|library)\s*\(\s*([A-Za-z0-9_\-${}]+)").unwrap();
            if !txt.contains("crossfire.cmake") {
                if let Some(c) = re.captures(&txt) {
                    let tgt = c[2].to_string();
                    let new = format!("{}\n# Added by Crossfire: portability layer, per-OS sources and libraries.\ninclude(${{CMAKE_CURRENT_SOURCE_DIR}}/crossfire/crossfire.cmake)\ncrossfire_configure({tgt})\n", txt.trim_end());
                    out.write("CMakeLists.txt", &new, ChangeMethod::Rule, 0.85, vec![format!("Appended include(crossfire/crossfire.cmake) + crossfire_configure({tgt})")])?;
                }
            }
        }
        "meson" => {
            out.write("meson.build", &templates::meson_build(plan), ChangeMethod::Template, 0.8, vec!["Generated Meson project with per-OS sources and dependencies".into()])?;
        }
        "keep" => {
            // No CMake to keep (e.g. MSBuild/Xcode/Make): still provide a
            // CMake build alongside so other OSes can build.
            write_cmake(out, plan, "Generated CMake build next to the existing build files")?;
        }
        _ => {
            if cmake_root_exists {
                let orig = std::fs::read_to_string(out.path("CMakeLists.txt")).unwrap_or_default();
                out.write("CMakeLists.original.txt", &orig, ChangeMethod::Copy, 1.0, vec!["Original CMakeLists.txt kept for reference".into()])?;
            }
            write_cmake(out, plan, "Generated cross-platform CMake project")?;
        }
    }
    Ok(())
}

fn write_cmake(out: &mut Output, plan: &NativePlan, note: &str) -> Result<()> {
    let mut notes = vec![note.to_string()];
    if !plan.affine.is_empty() {
        notes.push(format!("{} OS-specific source(s) compiled per platform", plan.affine.len()));
    }
    if plan.sdl3_default {
        notes.push(format!("SDL3 via find_package or FetchContent ({})", templates::SDL3_TAG));
    }
    out.write("CMakeLists.txt", &templates::cmake_lists(plan), ChangeMethod::Template, 0.85, notes)?;
    out.write("CMakePresets.json", &templates::cmake_presets(plan), ChangeMethod::Template, 1.0, vec!["Configure/build presets for each OS".into()])?;
    out.write("packaging/macos/Info.plist.in", &templates::render(templates::INFO_PLIST_IN, &[("CATEGORY", category(plan.gui, true))]), ChangeMethod::Template, 1.0, vec!["Bundle Info.plist template used by CMake (MACOSX_BUNDLE_INFO_PLIST)".into()])?;
    Ok(())
}

fn category(gui: bool, game: bool) -> &'static str {
    if gui && game {
        "public.app-category.games"
    } else {
        "public.app-category.developer-tools"
    }
}

fn generate_packaging(out: &mut Output, name: &str, display: &str, answers: &Answers, packages: &[String], report: &ScanReport) -> Result<()> {
    let game = report.kinds.iter().any(|k| k.flavor == "game");
    let gui = report.kinds.iter().any(|k| k.flavor == "game" || k.flavor == "app");
    let bundle = answers.str_or("bundleId", &format!("com.example.{}", name.to_lowercase()));
    let vars: Vec<(&str, &str)> = vec![
        ("NAME", name),
        ("DISPLAY", display),
        ("BUNDLE_ID", &bundle),
        ("CATEGORY", category(gui, game)),
        ("DESKTOP_CATEGORY", if game { "Game" } else { "Utility" }),
        ("TERMINAL", if gui { "false" } else { "true" }),
    ];
    let has = |p: &str| packages.iter().any(|x| x == p);
    if has("appimage") || has("tarball") {
        out.write(&format!("packaging/linux/{name}.desktop"), &templates::render(templates::DESKTOP, &vars), ChangeMethod::Template, 1.0, vec!["Freedesktop launcher".into()])?;
        out.write(&format!("packaging/linux/{name}.svg"), templates::ICON_SVG, ChangeMethod::Template, 1.0, vec!["Placeholder icon".into()])?;
    }
    if has("appimage") {
        out.write("packaging/linux/build-appimage.sh", &templates::render(templates::APPIMAGE_SH, &vars), ChangeMethod::Template, 1.0, vec!["AppImage via linuxdeploy".into()])?;
        out.set_executable("packaging/linux/build-appimage.sh");
    }
    if has("app") || has("dmg") {
        out.write("packaging/macos/Info.plist", &templates::render(templates::INFO_PLIST, &vars), ChangeMethod::Template, 1.0, vec!["Info.plist for script-built bundles".into()])?;
        out.write("packaging/macos/make-app.sh", &templates::render(templates::MAKE_APP_SH, &vars), ChangeMethod::Template, 1.0, vec![".app bundle + .dmg (hdiutil), ad-hoc codesign".into()])?;
        out.set_executable("packaging/macos/make-app.sh");
    }
    if has("zip") {
        out.write("packaging/windows/package-zip.ps1", &templates::render(templates::PACKAGE_ZIP_PS1, &vars), ChangeMethod::Template, 1.0, vec!["Portable zip of the Windows build".into()])?;
    }
    if has("nsis") {
        out.write("packaging/windows/installer.nsi", &templates::render(templates::INSTALLER_NSI, &vars), ChangeMethod::Template, 1.0, vec!["NSIS installer script".into()])?;
    }
    if has("freebsd-pkg") {
        out.write("packaging/freebsd/+MANIFEST", &templates::render(templates::FREEBSD_MANIFEST, &vars), ChangeMethod::Template, 1.0, vec!["pkg-create manifest".into()])?;
        out.write("packaging/freebsd/make-pkg.sh", &templates::render(templates::FREEBSD_PKG_SH, &vars), ChangeMethod::Template, 1.0, vec!["FreeBSD .pkg via pkg create".into()])?;
        out.set_executable("packaging/freebsd/make-pkg.sh");
    }
    Ok(())
}

/// Decide the project's primary build kind for CI/build.
pub fn build_kind(report: &ScanReport, files: &[FileInfo], answers: &Answers) -> BuildKind {
    let kind = |n: &str| report.kinds.iter().any(|k| k.name == n);
    if let Some(e) = ["Unity", "Unreal", "Godot"].iter().find(|e| kind(e)) {
        return BuildKind::Engine(e.to_string());
    }
    let lead = report.languages.first().map(|l| l.name.as_str()).unwrap_or("");
    let cfam = files.iter().any(|f| f.lang.map(|l| is_unit(l.id)).unwrap_or(false));
    if cfam && matches!(lead, "C" | "C++" | "Objective-C" | "Objective-C++" | "C/C++ Header") {
        return match answers.str_or("buildSystem", "cmake").as_str() {
            "meson" => BuildKind::Meson,
            "keep" if !kind("CMake") && kind("Make") && !kind("MSBuild") && !kind("Xcode") => BuildKind::Make,
            _ => BuildKind::CMake,
        };
    }
    if kind("Cargo") {
        return BuildKind::Cargo;
    }
    if kind(".NET") {
        let project = files
            .iter()
            .filter(|f| f.rel.ends_with(".csproj") || f.rel.ends_with(".fsproj"))
            .min_by_key(|f| f.rel.matches('/').count())
            .map(|f| f.rel.clone())
            .unwrap_or_default();
        return BuildKind::DotNet { project };
    }
    if kind("Go") {
        return BuildKind::Go;
    }
    if kind("Electron") || kind("Node") || kind("Tauri") {
        return BuildKind::Node;
    }
    if kind("Python") {
        let mut py: Vec<&FileInfo> = files.iter().filter(|f| f.lang.map(|l| l.id == "python").unwrap_or(false) && !f.rel.starts_with("crossfire/")).collect();
        py.sort_by_key(|f| (!f.has_entry, f.rel.matches('/').count(), !f.rel.ends_with("main.py"), std::cmp::Reverse(f.lines)));
        return BuildKind::Python { entry: py.first().map(|f| f.rel.clone()).unwrap_or_else(|| "main.py".into()) };
    }
    if cfam {
        return BuildKind::CMake;
    }
    if kind("Make") {
        return BuildKind::Make;
    }
    BuildKind::Other
}

// ---------------------------------------------------------------------------
// Report
// ---------------------------------------------------------------------------

struct ReportCtx<'a> {
    src: &'a ScanReport,
    post: &'a ScanReport,
    targets: &'a [Os],
    source_os: &'a [Os],
    answers: &'a Answers,
    changes: &'a [FileChange],
    manual: &'a [Finding],
    plan: &'a PlanInfo,
    kind: &'a BuildKind,
    ai_notes: &'a [String],
    ci: Option<&'a str>,
}

fn report_markdown(r: &ReportCtx) -> String {
    use std::fmt::Write as _;
    let mut s = String::new();
    let labels = |v: &[Os]| v.iter().map(|o| o.label()).collect::<Vec<_>>().join(", ");
    let _ = writeln!(s, "# Crossfire conversion report: {}\n", r.src.name);
    let _ = writeln!(s, "## Summary\n");
    let _ = writeln!(s, "| | |\n|---|---|");
    let _ = writeln!(s, "| Source OS | {} |", if r.source_os.is_empty() { "unknown".into() } else { labels(r.source_os) });
    let _ = writeln!(s, "| Targets | {} |", labels(r.targets));
    let _ = writeln!(s, "| Project kinds | {} |", r.src.kinds.iter().map(|k| format!("{} ({})", k.name, k.flavor)).collect::<Vec<_>>().join(", "));
    let _ = writeln!(s, "| Languages | {} |", r.src.languages.iter().take(6).map(|l| format!("{} ({} lines)", l.name, l.lines)).collect::<Vec<_>>().join(", "));
    let _ = writeln!(s, "| Portability | {:.0}% → {:.0}% |", r.src.portability, r.post.portability);
    let _ = writeln!(s, "| Findings | {} → {} (medium or worse: {}) |", r.src.findings.len(), r.post.findings.len(), r.manual.len());
    let modified = r.changes.iter().filter(|c| c.action == FileAction::Modified).count();
    let added = r.changes.iter().filter(|c| c.action == FileAction::Added).count();
    let copied = r.changes.iter().filter(|c| c.action == FileAction::Copied).count();
    let _ = writeln!(s, "| Files | {modified} modified, {added} added, {copied} copied unchanged |");
    let _ = writeln!(s, "| Build | {:?} |\n", r.kind);

    let _ = writeln!(s, "### Choices\n");
    let mut keys: Vec<&String> = r.answers.keys().collect();
    keys.sort();
    for k in keys {
        let _ = writeln!(s, "- **{k}**: {}", r.answers[k]);
    }
    s.push('\n');

    let _ = writeln!(s, "## Changes\n");
    for c in r.changes.iter().filter(|c| c.action != FileAction::Copied) {
        let _ = writeln!(s, "### `{}` — {:?} by {:?} (confidence {:.0}%)\n", c.path, c.action, c.method, c.confidence * 100.0);
        for n in &c.notes {
            let _ = writeln!(s, "- {n}");
        }
        s.push('\n');
    }
    if !r.plan.stubs.is_empty() {
        let _ = writeln!(s, "## OS-specific sources and stubs\n");
        let _ = writeln!(s, "These files are tied to one platform. They are compiled only there; elsewhere the generated stub is compiled so the project links. Each stubbed function prints a warning at runtime — implement it with the chosen portable API.\n");
        let _ = writeln!(s, "| Source | Native on | Stub | Functions |\n|---|---|---|---|");
        for (src, label, stub, names) in &r.plan.stubs {
            let _ = writeln!(s, "| `{src}` | {label} | `{stub}` | {} |", names.join(", "));
        }
        s.push('\n');
    }
    if !r.plan.plan.excluded.is_empty() {
        let _ = writeln!(s, "Additional entry points not compiled into the main target: {}\n", r.plan.plan.excluded.iter().map(|x| format!("`{x}`")).collect::<Vec<_>>().join(", "));
    }

    let _ = writeln!(s, "## Manual review ({} findings)\n", r.manual.len());
    let mut by_file: BTreeMap<&str, Vec<&Finding>> = BTreeMap::new();
    for f in r.manual {
        by_file.entry(f.file.as_str()).or_default().push(f);
    }
    for (file, fs) in by_file {
        let _ = writeln!(s, "### `{file}`\n");
        for f in fs.iter().take(60) {
            let sugg = r.targets.iter().filter(|t| !f.os.contains(t)).filter_map(|t| f.suggestions.get(t).map(|x| format!("{}: {x}", t.label()))).next().unwrap_or_default();
            let _ = writeln!(s, "- L{} **{:?}** `{}` — {} {}", f.line, f.severity, f.api, f.message, if sugg.is_empty() { String::new() } else { format!("→ {sugg}") });
        }
        if fs.len() > 60 {
            let _ = writeln!(s, "- … {} more", fs.len() - 60);
        }
        s.push('\n');
    }
    if !r.ai_notes.is_empty() {
        let _ = writeln!(s, "## AI pass\n");
        for n in r.ai_notes {
            let _ = writeln!(s, "- {n}");
        }
        s.push('\n');
    }

    let _ = writeln!(s, "## Next steps\n");
    match r.kind {
        BuildKind::CMake => {
            let _ = writeln!(s, "1. Configure and build on each OS:\n\n   ```sh\n   cmake --preset default        # or: cmake -S . -B build -DCMAKE_BUILD_TYPE=Release\n   cmake --build build/default\n   ```\n");
            if r.plan.plan.sdl3_default {
                let _ = writeln!(s, "   SDL3 is enabled (`-DCF_USE_SDL3=OFF` to build without it). A system SDL3 is used when found, otherwise it is fetched ({}).\n", templates::SDL3_TAG);
            }
        }
        BuildKind::Cargo => {
            let _ = writeln!(s, "1. `cargo build --release` on each OS (or `cargo zigbuild --target …` to cross-compile).\n");
        }
        BuildKind::DotNet { project } => {
            let _ = writeln!(s, "1. `dotnet publish {project} -c Release -r <win-x64|osx-arm64|linux-x64>`; WinForms/WPF UI must be ported (e.g. to Avalonia) before it runs off Windows.\n");
        }
        BuildKind::Python { entry } => {
            let _ = writeln!(s, "1. Run `python3 {entry}` on each OS; bundle with `pyinstaller --onefile {entry}`.\n");
        }
        _ => {
            let _ = writeln!(s, "1. Build with the project's tooling on each OS.\n");
        }
    }
    let _ = writeln!(s, "2. Work through the manual review list above, starting with blockers.");
    if !r.plan.stubs.is_empty() {
        let _ = writeln!(s, "3. Replace the generated stubs in `crossfire/stubs/` with real implementations.");
    }
    if let Some(ci) = r.ci {
        let _ = writeln!(s, "4. Push to GitHub: `{ci}` builds every target (FreeBSD in a VM) and uploads artifacts.");
    }
    let _ = writeln!(s, "5. Use **Build** in Crossfire to compile locally for the targets your machine supports; the rest are marked for CI.");
    if r.source_os.contains(&Os::Windows) && r.targets.contains(&Os::Macos) {
        let _ = writeln!(s, "6. On a Mac, the original Windows build can be smoke-tested with Game Porting Toolkit (Test → GPTK).");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_sanitized() {
        assert_eq!(sanitize_name("My Game!"), "My_Game");
        assert_eq!(sanitize_name("3d"), "app_3d");
    }

    #[test]
    fn normalize_paths() {
        assert_eq!(normalize("src/../include/./a.h"), "include/a.h");
    }
}
