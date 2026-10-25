//! Project scanning: walk the tree, classify files, run the rule engine,
//! detect project kinds/dependencies and compute a portability score.

use crate::detect;
use crate::lang::{self, Lang};
use crate::model::*;
use crate::rules;
use anyhow::{bail, Result};
use regex::Regex;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, LazyLock, Mutex};
use std::time::Instant;

/// Directory names never descended into (build output, dependencies, caches).
const SKIP_DIRS: &[&str] = &[
    ".git", ".hg", ".svn", "node_modules", "target", "build", "Build", "builds", "bin", "obj",
    "Library", ".godot", "DerivedData", "vendor", "third_party", "thirdparty", "3rdparty",
    "external", ".vs", ".idea", "out", "Pods", "Carthage", "__pycache__", ".venv", "venv",
    "dist", "Binaries", "Intermediate", "Saved", "DerivedDataCache", "Temp", "Logs", ".gradle",
    ".dart_tool", "zig-cache", ".zig-cache", "zig-out", ".cache", ".tox", ".mypy_cache",
    ".pytest_cache", ".build", ".swiftpm", "xcuserdata", ".import", "Packages_cache",
];

/// Largest file we read for analysis.
const MAX_TEXT_BYTES: u64 = 4 * 1024 * 1024;

/// Per-file facts collected during the scan (used by convert as well).
#[derive(Debug, Clone)]
pub struct FileInfo {
    /// Path relative to the root, always with forward slashes.
    pub rel: String,
    pub lang: Option<Lang>,
    pub lines: usize,
    /// Readable text (not binary, not oversized).
    pub text: bool,
    /// Contains a program entry point (main, WinMain, NSApplicationMain...).
    pub has_entry: bool,
}

/// Full scan output: the public report plus per-file details.
pub struct ScanData {
    pub report: ScanReport,
    pub files: Vec<FileInfo>,
    /// Directories skipped because they are bundled frameworks (*.framework).
    pub frameworks: Vec<String>,
}

pub fn should_skip_dir(name: &str) -> bool {
    SKIP_DIRS.contains(&name) || name.starts_with("cmake-build-") || name.ends_with(".framework")
}

/// Walk `root` honoring .gitignore and the skip list. Returns relative paths
/// of files and the list of skipped *.framework directories.
pub fn walk(root: &Path) -> (Vec<PathBuf>, Vec<String>) {
    let frameworks: Arc<Mutex<Vec<String>>> = Arc::default();
    let fw = frameworks.clone();
    let root_owned = root.to_path_buf();
    let walker = ignore::WalkBuilder::new(root)
        .hidden(false)
        .git_ignore(true)
        .git_exclude(true)
        .git_global(false)
        .require_git(false)
        .parents(false)
        .ignore(true)
        .filter_entry(move |e| {
            if e.depth() == 0 {
                return true;
            }
            let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if !is_dir {
                return true;
            }
            let name = e.file_name().to_string_lossy();
            if name.ends_with(".framework") {
                if let Ok(rel) = e.path().strip_prefix(&root_owned) {
                    fw.lock().unwrap_or_else(|p| p.into_inner()).push(rel_string(rel));
                }
                return false;
            }
            !should_skip_dir(&name)
        })
        .build();
    let mut files = Vec::new();
    for entry in walker.flatten() {
        if entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            if let Ok(rel) = entry.path().strip_prefix(root) {
                files.push(rel.to_path_buf());
            }
        }
    }
    files.sort();
    let fws = frameworks.lock().unwrap_or_else(|p| p.into_inner()).clone();
    (files, fws)
}

pub fn rel_string(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

/// Read a file as text if it is small and not binary.
pub fn read_text(path: &Path) -> Option<String> {
    let meta = std::fs::metadata(path).ok()?;
    if meta.len() > MAX_TEXT_BYTES {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    let head = &bytes[..bytes.len().min(8192)];
    if head.contains(&0) {
        return None;
    }
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

fn entry_regex(lang_id: &str) -> Option<&'static Regex> {
    static C: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^\s*(int|void|auto)\s+main\s*\(|\b(WinMain|wWinMain|_tWinMain|NSApplicationMain|UIApplicationMain|SDL_AppInit)\s*\(").unwrap()
    });
    static RS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^\s*(pub\s+)?fn\s+main\s*\(").unwrap());
    static GO: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^func\s+main\s*\(").unwrap());
    static CS: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\bstatic\s+(async\s+)?(void|int|Task(<int>)?)\s+Main\s*\(").unwrap()
    });
    static JAVA: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"public\s+static\s+void\s+main\s*\(|(?m)^fun\s+main\s*\(").unwrap()
    });
    static PY: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"if\s+__name__\s*==\s*['"]__main__['"]"#).unwrap());
    static SWIFT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"@main\b|NSApplicationMain").unwrap());
    match lang_id {
        "c" | "cpp" | "objc" | "objcpp" => Some(&C),
        "rust" => Some(&RS),
        "go" => Some(&GO),
        "csharp" => Some(&CS),
        "java" | "kotlin" => Some(&JAVA),
        "python" => Some(&PY),
        "swift" => Some(&SWIFT),
        _ => None,
    }
}

struct FileResult {
    info: FileInfo,
    findings: Vec<Finding>,
}

fn analyze(root: &Path, rel: &Path) -> FileResult {
    let rel_s = rel_string(rel);
    let lang = lang::classify(rel);
    let mut info = FileInfo { rel: rel_s.clone(), lang, lines: 0, text: false, has_entry: false };
    let mut findings = Vec::new();
    if let Some(l) = lang {
        if let Some(text) = read_text(&root.join(rel)) {
            info.text = true;
            info.lines = text.lines().count();
            info.has_entry = entry_regex(l.id).map(|re| re.is_match(&text)).unwrap_or(false);
            findings = rules::scan_text(&rel_s, l.id, &text);
        }
    }
    FileResult { info, findings }
}

/// Public scan entry point.
pub fn scan(root: &Path, on: &dyn Fn(ProgressEvent)) -> Result<ScanReport> {
    Ok(scan_full(root, on)?.report)
}

/// Scan returning per-file details too.
pub fn scan_full(root: &Path, on: &dyn Fn(ProgressEvent)) -> Result<ScanData> {
    let started = Instant::now();
    if !root.is_dir() {
        bail!("{} is not a directory", root.display());
    }
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    on(ProgressEvent::new(Stage::Scan, Level::Info, 0.0, format!("Walking {}", root.display())));
    let (paths, frameworks) = walk(&root);
    let total = paths.len();
    on(ProgressEvent::new(Stage::Scan, Level::Info, 0.05, format!("Found {total} files; analyzing")));

    // Analyze files in parallel; progress flows back over a channel so the
    // (non-Sync) callback is only ever invoked from this thread.
    let workers = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(1, 16);
    let chunk = total.div_ceil(workers).max(1);
    let (tx, rx) = mpsc::channel::<String>();
    let mut results: Vec<FileResult> = Vec::with_capacity(total);
    std::thread::scope(|s| {
        let handles: Vec<_> = paths
            .chunks(chunk)
            .map(|part| {
                let tx = tx.clone();
                let root = &root;
                s.spawn(move || {
                    part.iter()
                        .map(|p| {
                            let r = analyze(root, p);
                            let _ = tx.send(r.info.rel.clone());
                            r
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        drop(tx);
        let step = (total / 50).max(1);
        for (done, rel) in rx.iter().enumerate() {
            if (done + 1) % step == 0 || done + 1 == total {
                let p = 0.05 + 0.8 * (done + 1) as f64 / total.max(1) as f64;
                on(ProgressEvent::new(Stage::Scan, Level::Info, p, format!("Analyzed {}/{}", done + 1, total)).with_file(rel));
            }
        }
        for h in handles {
            if let Ok(mut v) = h.join() {
                results.append(&mut v);
            }
        }
    });
    results.sort_by(|a, b| a.info.rel.cmp(&b.info.rel));

    let mut files = Vec::with_capacity(results.len());
    let mut findings = Vec::new();
    for r in results {
        files.push(r.info);
        findings.extend(r.findings);
    }
    // Stable, unique ids: "<rule>#<n>".
    for (i, f) in findings.iter_mut().enumerate() {
        f.id = format!("{}#{}", f.id, i + 1);
    }

    on(ProgressEvent::new(Stage::Scan, Level::Info, 0.9, "Detecting project kinds and dependencies"));
    let det = detect::detect(&root, &files, &findings, &frameworks);

    let languages = language_stats(&files);
    let total_lines = files.iter().filter(|f| f.lang.is_some()).map(|f| f.lines).sum();
    let portability = portability(&files, &findings);
    let mut categories: BTreeMap<Category, usize> = BTreeMap::new();
    for f in findings.iter().filter(|f| f.severity >= Severity::Low) {
        *categories.entry(f.category).or_default() += 1;
    }
    let detected_os = detect_os(&findings, &det.os_hints);
    let name = det.name.clone().unwrap_or_else(|| {
        root.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "project".into())
    });

    let report = ScanReport {
        root: root.to_string_lossy().to_string(),
        name,
        detected_os,
        languages,
        kinds: det.kinds,
        total_files: files.len(),
        total_lines,
        portability,
        findings,
        categories,
        dependencies: det.dependencies,
        duration_ms: started.elapsed().as_millis() as u64,
    };
    on(ProgressEvent::new(
        Stage::Scan,
        Level::Success,
        1.0,
        format!(
            "Scanned {} files, {} findings, portability {:.0}%",
            report.total_files,
            report.findings.len(),
            report.portability
        ),
    ));
    Ok(ScanData { report, files, frameworks })
}

fn language_stats(files: &[FileInfo]) -> Vec<LanguageStat> {
    let mut map: HashMap<&'static str, (usize, usize)> = HashMap::new();
    for f in files {
        if let Some(l) = f.lang {
            if matches!(l.id, "manifest" | "plist" | "winrc" | "godot") {
                continue;
            }
            let e = map.entry(l.name).or_default();
            e.0 += 1;
            e.1 += f.lines;
        }
    }
    // Attribute .h headers to C++ or C depending on which dominates.
    if let Some(h) = map.remove("C/C++ Header") {
        let cpp = map.get("C++").map(|x| x.0).unwrap_or(0) + map.get("Objective-C++").map(|x| x.0).unwrap_or(0);
        let c = map.get("C").map(|x| x.0).unwrap_or(0) + map.get("Objective-C").map(|x| x.0).unwrap_or(0);
        let target = if cpp >= c && cpp > 0 {
            "C++"
        } else if c > 0 && map.contains_key("C") {
            "C"
        } else if map.contains_key("Objective-C") {
            "Objective-C"
        } else {
            "C/C++ Header"
        };
        let e = map.entry(target).or_default();
        e.0 += h.0;
        e.1 += h.1;
    }
    let mut v: Vec<LanguageStat> =
        map.into_iter().map(|(n, (files, lines))| LanguageStat { name: n.to_string(), files, lines }).collect();
    v.sort_by(|a, b| b.lines.cmp(&a.lines).then(a.name.cmp(&b.name)));
    v
}

/// Share of source lines living in files without medium+ findings, with
/// medium/high findings counting partially (see `rules::severity_weight`).
pub fn portability(files: &[FileInfo], findings: &[Finding]) -> f64 {
    let mut worst: HashMap<&str, Severity> = HashMap::new();
    for f in findings {
        // APIs available on all four OSes do not hurt portability.
        if f.os.len() == Os::ALL.len() {
            continue;
        }
        let e = worst.entry(f.file.as_str()).or_insert(Severity::Info);
        if f.severity > *e {
            *e = f.severity;
        }
    }
    let mut total = 0.0;
    let mut bad = 0.0;
    for f in files.iter().filter(|f| f.lang.map(|l| l.code).unwrap_or(false)) {
        let lines = f.lines.max(1) as f64;
        total += lines;
        if let Some(s) = worst.get(f.rel.as_str()) {
            bad += lines * rules::severity_weight(*s);
        }
    }
    if total == 0.0 {
        return 100.0;
    }
    ((1.0 - bad / total) * 1000.0).round() / 10.0
}

/// Infer which OSes the code currently targets from OS-specific findings and
/// project-file hints.
pub fn detect_os(findings: &[Finding], hints: &[(Os, f64)]) -> Vec<Os> {
    let mut score: BTreeMap<Os, f64> = BTreeMap::new();
    let mut unix_score = 0.0;
    for f in findings {
        let w = match f.severity {
            Severity::Blocker => 5.0,
            Severity::High => 3.0,
            Severity::Medium => 2.0,
            Severity::Low => 1.0,
            Severity::Info => 0.5,
        };
        match f.os.len() {
            1 | 2 => {
                for o in &f.os {
                    *score.entry(*o).or_default() += w / f.os.len() as f64;
                }
            }
            3 => unix_score += w,
            _ => {}
        }
    }
    for (o, w) in hints {
        *score.entry(*o).or_default() += w;
    }
    let max = score.values().cloned().fold(0.0, f64::max);
    if max <= 0.0 {
        return if unix_score > 0.0 { vec![Os::Macos, Os::Linux, Os::Freebsd] } else { Os::ALL.to_vec() };
    }
    let mut v: Vec<Os> = score.into_iter().filter(|(_, s)| *s >= max * 0.35).map(|(o, _)| o).collect();
    v.sort();
    v
}
