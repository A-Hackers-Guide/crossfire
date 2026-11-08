//! Data-driven rule engine.
//!
//! Rules live in embedded TOML tables (one file per platform family). Each
//! table is a list of groups that share defaults (languages, OS, category,
//! severity); individual rules override what they need. At first use the
//! tables are parsed and compiled into regexes once.

use crate::lang;
use crate::model::{Category, Finding, Os, Severity};
use regex::{Regex, RegexBuilder};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::LazyLock;

/// Embedded rule tables. Order only matters for id uniqueness checks.
const TABLES: &[(&str, &str)] = &[
    ("win32", include_str!("win32.toml")),
    ("directx", include_str!("directx.toml")),
    ("macos", include_str!("macos.toml")),
    ("linux", include_str!("linux.toml")),
    ("bsd", include_str!("bsd.toml")),
    ("posix", include_str!("posix.toml")),
    ("languages", include_str!("languages.toml")),
    ("engines", include_str!("engines.toml")),
    ("build", include_str!("build.toml")),
];

#[derive(Deserialize)]
struct RawTable {
    group: Vec<RawGroup>,
}

#[derive(Deserialize)]
struct RawGroup {
    lang: Vec<String>,
    os: Vec<Os>,
    cat: Category,
    sev: Severity,
    #[serde(default)]
    fix: Option<String>,
    rules: Vec<RawRule>,
}

#[derive(Deserialize)]
struct RawRule {
    id: String,
    api: String,
    re: String,
    msg: String,
    #[serde(default)]
    fix: Option<String>,
    #[serde(default)]
    sev: Option<Severity>,
    #[serde(default)]
    cat: Option<Category>,
    #[serde(default)]
    os: Option<Vec<Os>>,
    #[serde(default)]
    lang: Option<Vec<String>>,
    /// Case-insensitive match.
    #[serde(default)]
    icase: bool,
    /// Report only the first match in a file.
    #[serde(default)]
    once: bool,
    /// Line must NOT match this regex for the rule to fire.
    #[serde(default)]
    not: Option<String>,
    /// Also match inside comment lines (e.g. for directives in comments).
    #[serde(default)]
    comments: bool,
    #[serde(default)]
    windows: Option<String>,
    #[serde(default)]
    macos: Option<String>,
    #[serde(default)]
    linux: Option<String>,
    #[serde(default)]
    freebsd: Option<String>,
}

/// A compiled rule.
#[derive(Debug)]
pub struct Rule {
    pub id: String,
    pub table: &'static str,
    pub langs: Vec<&'static str>,
    pub re: Regex,
    pub not: Option<Regex>,
    pub category: Category,
    pub severity: Severity,
    pub os: Vec<Os>,
    pub api: String,
    pub message: String,
    pub once: bool,
    pub comments: bool,
    pub suggestions: BTreeMap<Os, String>,
}

impl Rule {
    pub fn applies_to(&self, lang_id: &str) -> bool {
        self.langs.iter().any(|l| *l == lang_id || *l == "any")
    }
}

/// All compiled rules (panics at first use if an embedded table is malformed —
/// that is a programming error caught by the unit tests).
pub static RULES: LazyLock<Vec<Rule>> = LazyLock::new(|| load().expect("embedded rule tables"));

pub fn rules() -> &'static [Rule] {
    &RULES
}

fn load() -> anyhow::Result<Vec<Rule>> {
    let mut out = Vec::new();
    for (table, src) in TABLES {
        let parsed: RawTable =
            toml::from_str(src).map_err(|e| anyhow::anyhow!("rule table {table}: {e}"))?;
        for g in parsed.group {
            for r in g.rules {
                let langs: Vec<&'static str> = r
                    .lang
                    .as_ref()
                    .unwrap_or(&g.lang)
                    .iter()
                    .flat_map(|t| lang::expand_token(t))
                    .collect();
                let os = r.os.clone().unwrap_or_else(|| g.os.clone());
                let pattern = if r.icase {
                    format!("(?i){}", r.re)
                } else {
                    r.re.clone()
                };
                let re = RegexBuilder::new(&pattern)
                    .size_limit(16 << 20)
                    .build()
                    .map_err(|e| anyhow::anyhow!("rule {}: bad regex: {e}", r.id))?;
                let not = match &r.not {
                    Some(n) => Some(
                        Regex::new(n)
                            .map_err(|e| anyhow::anyhow!("rule {}: bad not: {e}", r.id))?,
                    ),
                    None => None,
                };
                let fix = r.fix.clone().or_else(|| g.fix.clone());
                let suggestions = build_suggestions(&os, fix.as_deref(), &r);
                out.push(Rule {
                    id: r.id,
                    table,
                    langs,
                    re,
                    not,
                    category: r.cat.unwrap_or(g.cat),
                    severity: r.sev.unwrap_or(g.sev),
                    os,
                    api: r.api,
                    message: r.msg,
                    once: r.once,
                    comments: r.comments,
                    suggestions,
                });
            }
        }
    }
    Ok(out)
}

/// Per-target suggestions: explicit per-OS text wins, then the generic `fix`
/// for OSes the API does not exist on, and a "keep it guarded" note for the
/// OSes where it is native.
fn build_suggestions(os: &[Os], fix: Option<&str>, r: &RawRule) -> BTreeMap<Os, String> {
    let mut m = BTreeMap::new();
    for target in Os::ALL {
        let explicit = match target {
            Os::Windows => r.windows.as_deref(),
            Os::Macos => r.macos.as_deref(),
            Os::Linux => r.linux.as_deref(),
            Os::Freebsd => r.freebsd.as_deref(),
        };
        let text = if let Some(e) = explicit {
            Some(e.to_string())
        } else if os.contains(&target) {
            if os.len() == Os::ALL.len() {
                fix.map(str::to_string)
            } else {
                Some(format!(
                    "Native on {}; keep it behind a platform guard ({}).",
                    target.label(),
                    target.c_macro()
                ))
            }
        } else {
            fix.map(str::to_string)
        };
        if let Some(t) = text {
            m.insert(target, t);
        }
    }
    m
}

// ---------------------------------------------------------------------------
// Preprocessor guard tracking (C family)
// ---------------------------------------------------------------------------

/// OS macros recognised in `#if` conditions.
fn macro_os(cond: &str) -> Vec<Os> {
    static RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\b(_WIN32|_WIN64|WIN32|_WINDOWS|_MSC_VER|__MINGW32__|__MINGW64__|__CYGWIN__|__APPLE__|__MACH__|TARGET_OS_OSX|TARGET_OS_MAC|TARGET_OS_IPHONE|__linux__|__linux|__gnu_linux__|__ANDROID__|__FreeBSD__|__DragonFly__|__unix__|__unix|_POSIX_VERSION|SDL_PLATFORM_WINDOWS|SDL_PLATFORM_MACOS|SDL_PLATFORM_LINUX|SDL_PLATFORM_FREEBSD)\b").unwrap()
    });
    let mut v = Vec::new();
    for m in RE.find_iter(cond) {
        let add: &[Os] = match m.as_str() {
            "_WIN32"
            | "_WIN64"
            | "WIN32"
            | "_WINDOWS"
            | "_MSC_VER"
            | "__MINGW32__"
            | "__MINGW64__"
            | "__CYGWIN__"
            | "SDL_PLATFORM_WINDOWS" => &[Os::Windows],
            "__APPLE__" | "__MACH__" | "TARGET_OS_OSX" | "TARGET_OS_MAC" | "TARGET_OS_IPHONE"
            | "SDL_PLATFORM_MACOS" => &[Os::Macos],
            "__linux__" | "__linux" | "__gnu_linux__" | "__ANDROID__" | "SDL_PLATFORM_LINUX" => {
                &[Os::Linux]
            }
            "__FreeBSD__" | "__DragonFly__" | "SDL_PLATFORM_FREEBSD" => &[Os::Freebsd],
            // __unix__ is defined on Linux and the BSDs but not on macOS.
            "__unix__" | "__unix" => &[Os::Linux, Os::Freebsd],
            "_POSIX_VERSION" => &[Os::Linux, Os::Freebsd, Os::Macos],
            _ => &[],
        };
        for o in add {
            if !v.contains(o) {
                v.push(*o);
            }
        }
    }
    v
}

#[derive(Clone)]
struct Frame {
    /// OSes allowed in the current branch of this frame (None = unconstrained).
    allowed: Option<Vec<Os>>,
    /// OS set named by the condition and whether it was negated.
    named: Option<(Vec<Os>, bool)>,
}

fn complement(v: &[Os]) -> Vec<Os> {
    Os::ALL.iter().copied().filter(|o| !v.contains(o)).collect()
}

/// For each line, the set of OSes the line can be compiled for according to
/// enclosing `#if` guards (None = no OS guard in effect).
pub fn guard_map(text: &str) -> Vec<Option<Vec<Os>>> {
    static DIRECTIVE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^\s*#\s*(ifdef|ifndef|if|elif|else|endif)\b(.*)$").unwrap());
    let mut stack: Vec<Frame> = Vec::new();
    let mut out = Vec::new();
    for line in text.lines() {
        // The directive line itself is attributed to the outer context.
        out.push(effective(&stack));
        if let Some(c) = DIRECTIVE.captures(line) {
            let kw = &c[1];
            let rest = c.get(2).map(|m| m.as_str()).unwrap_or("");
            match kw {
                "ifdef" | "ifndef" | "if" => {
                    let oses = macro_os(rest);
                    let negated = kw == "ifndef" || rest.trim_start().starts_with('!');
                    if oses.is_empty() {
                        stack.push(Frame {
                            allowed: None,
                            named: None,
                        });
                    } else {
                        let allowed = if negated {
                            complement(&oses)
                        } else {
                            oses.clone()
                        };
                        stack.push(Frame {
                            allowed: Some(allowed),
                            named: Some((oses, negated)),
                        });
                    }
                }
                "elif" => {
                    if let Some(f) = stack.last_mut() {
                        let oses = macro_os(rest);
                        if oses.is_empty() {
                            // Unknown elif: whatever wasn't taken before.
                            f.allowed =
                                f.named
                                    .as_ref()
                                    .map(|(o, n)| if *n { o.clone() } else { complement(o) });
                        } else {
                            let negated = rest.trim_start().starts_with('!');
                            f.allowed = Some(if negated {
                                complement(&oses)
                            } else {
                                oses.clone()
                            });
                            // Accumulate what has been "used" for a later #else.
                            let prev = f
                                .named
                                .take()
                                .map(|(o, n)| if n { complement(&o) } else { o })
                                .unwrap_or_default();
                            let mut used = prev;
                            for o in f.allowed.clone().unwrap_or_default() {
                                if !used.contains(&o) {
                                    used.push(o);
                                }
                            }
                            f.named = Some((used, false));
                        }
                    }
                }
                "else" => {
                    if let Some(f) = stack.last_mut() {
                        f.allowed =
                            f.named
                                .as_ref()
                                .map(|(o, n)| if *n { o.clone() } else { complement(o) });
                    }
                }
                "endif" => {
                    stack.pop();
                }
                _ => {}
            }
        }
    }
    out
}

fn effective(stack: &[Frame]) -> Option<Vec<Os>> {
    let mut acc: Option<Vec<Os>> = None;
    for f in stack {
        if let Some(a) = &f.allowed {
            acc = Some(match acc {
                None => a.clone(),
                Some(prev) => prev.into_iter().filter(|o| a.contains(o)).collect(),
            });
        }
    }
    acc
}

// ---------------------------------------------------------------------------
// Matching
// ---------------------------------------------------------------------------

/// Rules applicable to one language plus a combined RegexSet for fast
/// line pre-filtering.
struct LangRules {
    rules: Vec<usize>,
    set: regex::RegexSet,
}

fn lang_rules(lang_id: &str) -> std::sync::Arc<LangRules> {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    static CACHE: LazyLock<Mutex<HashMap<String, Arc<LangRules>>>> =
        LazyLock::new(Default::default);
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(lr) = cache.get(lang_id) {
        return lr.clone();
    }
    let idx: Vec<usize> = rules()
        .iter()
        .enumerate()
        .filter(|(_, r)| r.applies_to(lang_id))
        .map(|(i, _)| i)
        .collect();
    // Case-insensitive rules carry an inline (?i) flag, so as_str() is exact.
    let set = regex::RegexSetBuilder::new(idx.iter().map(|&i| rules()[i].re.as_str()))
        .size_limit(64 << 20)
        .build()
        .expect("rule regexes compile individually, so the set compiles");
    let lr = Arc::new(LangRules { rules: idx, set });
    cache.insert(lang_id.to_string(), lr.clone());
    lr
}

/// Max findings one rule may produce in one file (keeps reports readable).
const PER_RULE_FILE_CAP: usize = 40;

/// Is this line a comment for the given language?
pub fn is_comment(lang_id: &str, line: &str) -> bool {
    let t = line.trim_start();
    if lang_id == "python" && t.starts_with("#!") {
        return true;
    }
    lang::comment_prefixes(lang_id)
        .iter()
        .any(|p| t.starts_with(p))
}

/// Apply all rules to one file. `rel` is the path shown in findings.
/// Findings get ids `<rule>#<n>` where n is assigned by the caller later.
pub fn scan_text(rel: &str, lang_id: &str, text: &str) -> Vec<Finding> {
    let lr = lang_rules(lang_id);
    if lr.rules.is_empty() {
        return Vec::new();
    }
    let applicable: Vec<&Rule> = lr.rules.iter().map(|&i| &rules()[i]).collect();
    let guards = if lang::is_c_family(lang_id) {
        guard_map(text)
    } else {
        Vec::new()
    };
    let mut counts = vec![0usize; applicable.len()];
    let mut out = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        if line.len() > 4000 {
            continue; // minified / generated
        }
        // One pass over the line with the combined set, then confirm per rule.
        let hits = lr.set.matches(line);
        if !hits.matched_any() {
            continue;
        }
        let comment = is_comment(lang_id, line);
        for ri in hits.iter() {
            let rule = applicable[ri];
            if (comment && !rule.comments)
                || counts[ri] >= PER_RULE_FILE_CAP
                || (rule.once && counts[ri] > 0)
            {
                continue;
            }
            if let Some(n) = &rule.not {
                if n.is_match(line) {
                    continue;
                }
            }
            counts[ri] += 1;
            let mut severity = rule.severity;
            let mut message = rule.message.clone();
            // Already guarded for the OS(es) this API belongs to → informational.
            if let Some(Some(allowed)) = guards.get(idx) {
                if !allowed.is_empty()
                    && allowed.iter().all(|o| rule.os.contains(o))
                    && rule.os.len() < 4
                {
                    severity = Severity::Info;
                    message.push_str(" (already behind a platform #if guard)");
                }
            }
            out.push(Finding {
                id: rule.id.clone(),
                file: rel.to_string(),
                line: idx + 1,
                snippet: snippet(line),
                category: rule.category,
                severity,
                os: rule.os.clone(),
                api: rule.api.clone(),
                message,
                suggestions: rule.suggestions.clone(),
            });
        }
    }
    out
}

fn snippet(line: &str) -> String {
    let t = line.trim();
    if t.chars().count() > 200 {
        let s: String = t.chars().take(197).collect();
        format!("{s}...")
    } else {
        t.to_string()
    }
}

/// Weight of a file's worst finding when computing portability: the share of
/// its lines considered non-portable.
pub fn severity_weight(s: Severity) -> f64 {
    match s {
        Severity::Info | Severity::Low => 0.0,
        Severity::Medium => 0.5,
        Severity::High => 0.85,
        Severity::Blocker => 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_load_and_ids_are_unique() {
        let rs = rules();
        assert!(rs.len() >= 250, "only {} rules", rs.len());
        let mut ids: Vec<&str> = rs.iter().map(|r| r.id.as_str()).collect();
        ids.sort();
        let before = ids.len();
        ids.dedup();
        assert_eq!(before, ids.len(), "duplicate rule ids");
        for r in rs {
            assert!(!r.langs.is_empty(), "{} has no langs", r.id);
            assert!(!r.os.is_empty(), "{} has no os", r.id);
            assert!(!r.suggestions.is_empty(), "{} has no suggestions", r.id);
        }
    }

    #[test]
    fn finds_win32_calls() {
        let src = "#include <windows.h>\nint main(){ Sleep(10); DWORD t = GetTickCount(); }\n";
        let f = scan_text("main.cpp", "cpp", src);
        let apis: Vec<&str> = f.iter().map(|x| x.api.as_str()).collect();
        assert!(apis.contains(&"windows.h"), "{apis:?}");
        assert!(apis.contains(&"Sleep"), "{apis:?}");
        assert!(apis.contains(&"GetTickCount"), "{apis:?}");
        assert!(f
            .iter()
            .all(|x| x.os.contains(&Os::Windows) || x.os.len() == 4));
    }

    #[test]
    fn comments_are_skipped() {
        let f = scan_text("a.c", "c", "// Sleep(10);\n/* CreateWindowEx */\n");
        assert!(f.is_empty(), "{f:?}");
    }

    #[test]
    fn guarded_code_is_informational() {
        let src = "#ifdef _WIN32\n  Sleep(5);\n#else\n  usleep(5000);\n#endif\n";
        let f = scan_text("a.c", "c", src);
        let sleep = f.iter().find(|x| x.api == "Sleep").expect("Sleep finding");
        assert_eq!(sleep.severity, Severity::Info);
        if let Some(u) = f.iter().find(|x| x.api == "usleep") {
            assert_eq!(
                u.severity,
                Severity::Info,
                "usleep is in the non-Windows branch"
            );
        }
    }

    #[test]
    fn guard_map_handles_else_and_nesting() {
        let src = "#if defined(__linux__) || defined(__FreeBSD__)\na\n#elif defined(__APPLE__)\nb\n#else\nc\n#endif\nd\n";
        let g = guard_map(src);
        assert_eq!(g[1], Some(vec![Os::Linux, Os::Freebsd]));
        assert_eq!(g[3], Some(vec![Os::Macos]));
        assert_eq!(g[5], Some(vec![Os::Windows]));
        assert_eq!(g[7], None);
    }

    #[test]
    fn linux_only_ifdef_flags_freebsd_gap() {
        let f = scan_text(
            "a.c",
            "c",
            "#ifdef __linux__\n#include <sys/epoll.h>\n#endif\n",
        );
        assert!(f.iter().any(|x| x.id == "bsd.linux-only-ifdef"), "{f:?}");
        let g = scan_text(
            "a.c",
            "c",
            "#if defined(__linux__) || defined(__FreeBSD__)\n#endif\n",
        );
        assert!(!g.iter().any(|x| x.id == "bsd.linux-only-ifdef"));
    }

    #[test]
    fn language_rules_fire() {
        let py = scan_text("a.py", "python", "import winreg\nos.startfile('x')\n");
        assert!(py.iter().any(|x| x.api == "winreg"));
        assert!(py.iter().any(|x| x.api == "os.startfile"));
        let cs = scan_text(
            "a.cs",
            "csharp",
            "[DllImport(\"user32.dll\")]\nusing System.Windows.Forms;\n",
        );
        assert!(cs.iter().any(|x| x.api.contains("user32")));
        assert!(cs.iter().any(|x| x.api.contains("WinForms")));
        let objc = scan_text(
            "a.m",
            "objc",
            "#import <Cocoa/Cocoa.h>\nid<MTLDevice> d = MTLCreateSystemDefaultDevice();\n",
        );
        assert!(objc
            .iter()
            .any(|x| x.os == vec![Os::Macos] && x.category == Category::Graphics));
        let x11 = scan_text(
            "w.c",
            "c",
            "#include <X11/Xlib.h>\nDisplay *d = XOpenDisplay(NULL);\n",
        );
        assert!(x11.iter().any(|x| x.api == "XOpenDisplay"));
    }

    #[test]
    fn suggestions_cover_targets() {
        let r = rules().iter().find(|r| r.id == "win32.sleep").unwrap();
        assert!(r.suggestions.contains_key(&Os::Linux));
        assert!(r.suggestions.contains_key(&Os::Macos));
        assert!(r.suggestions.contains_key(&Os::Freebsd));
    }
}
