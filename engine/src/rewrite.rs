//! Rule-based mechanical rewrites for C/C++, Python, C#, project files and
//! shell scripts. Every rewrite here is local and behavior-preserving; anything
//! riskier is left to the AI pass or manual review.

use crate::model::Os;
use crate::rules;
use regex::{Captures, Regex};
use std::collections::HashMap;
use std::sync::LazyLock;

/// Options shared by all rewrites of one conversion.
pub struct RewriteCtx<'a> {
    pub targets: &'a [Os],
    pub idiomatic: bool,
    pub fix_include_case: bool,
    /// lower-case relative path -> actual relative path, for include fixing.
    pub files_lower: &'a HashMap<String, String>,
    /// Directories (relative) that contain headers, searched for includes.
    pub include_dirs: &'a [String],
}

pub struct Rewritten {
    pub text: String,
    pub notes: Vec<String>,
}

fn re(s: &str) -> Regex {
    Regex::new(s).expect("static regex")
}

/// "../" prefix leading from `rel`'s directory back to the project root.
pub fn up_prefix(rel: &str) -> String {
    "../".repeat(rel.matches('/').count())
}

/// Headers that only exist on Windows and are superseded by cf_platform.h elsewhere.
const WIN_HEADERS: &[&str] = &[
    "windows.h",
    "windowsx.h",
    "tchar.h",
    "conio.h",
    "io.h",
    "direct.h",
    "process.h",
    "mmsystem.h",
    "timeapi.h",
    "winsock2.h",
    "winsock.h",
    "ws2tcpip.h",
    "mswsock.h",
    "shlobj.h",
    "shlobj_core.h",
    "shellapi.h",
    "shlwapi.h",
    "intrin.h",
    "objbase.h",
    "combaseapi.h",
    "versionhelpers.h",
    "d3d9.h",
    "d3d10.h",
    "d3d10_1.h",
    "d3d11.h",
    "d3d11_1.h",
    "d3d12.h",
    "dxgi.h",
    "dxgi1_2.h",
    "dxgi1_4.h",
    "dxgi1_6.h",
    "d3dcompiler.h",
    "xinput.h",
    "dinput.h",
    "dsound.h",
    "xaudio2.h",
    "directxmath.h",
    "wrl.h",
    "wrl/client.h",
    "commctrl.h",
    "commdlg.h",
    "dbghelp.h",
    "malloc.h",
    "basetsd.h",
    "crtdbg.h",
];
/// POSIX headers cf_platform.h replaces when compiling with MSVC.
const POSIX_SHIMMED: &[&str] = &["unistd.h", "strings.h", "sys/time.h"];

static INCLUDE: LazyLock<Regex> =
    LazyLock::new(|| re(r#"^(\s*#\s*include\s*)([<"])([^>"]+)([>"])(.*)$"#));
static PRAGMA_MSVC: LazyLock<Regex> =
    LazyLock::new(|| re(r"^\s*#\s*pragma\s+(comment\s*\(\s*lib|warning\s*\()"));
static SLEEP: LazyLock<Regex> = LazyLock::new(|| re(r"(^|[^\w:.>])Sleep\s*\("));
static TICKS: LazyLock<Regex> = LazyLock::new(|| re(r"(^|[^\w:.>])GetTickCount(64)?\s*\(\s*\)"));
static STRICMP: LazyLock<Regex> =
    LazyLock::new(|| re(r"(^|[^\w:.>])_?(stricmp|strnicmp|_stricmp|_strnicmp)\s*\("));
static STRCASECMP: LazyLock<Regex> =
    LazyLock::new(|| re(r"(^|[^\w:.>])(strcasecmp|strncasecmp)\s*\("));
static SNPRINTF: LazyLock<Regex> = LazyLock::new(|| re(r"(^|[^\w:.>])_(snprintf|vsnprintf)\s*\("));
static USLEEP: LazyLock<Regex> = LazyLock::new(|| re(r"(^|[^\w:.>])usleep\s*\("));
static DECLSPEC: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r"__declspec\s*\(\s*(dllexport|dllimport|noinline|noreturn|thread|align\s*\(\s*(\d+)\s*\))\s*\)",
    )
});
static FORCEINLINE: LazyLock<Regex> = LazyLock::new(|| re(r"\b__forceinline\b"));
static GETENV: LazyLock<Regex> = LazyLock::new(|| {
    re(r#"\b(getenv|_wgetenv)\s*\(\s*L?"(APPDATA|LOCALAPPDATA|USERPROFILE|TEMP|TMP)"\s*\)"#)
});
static PROC_EXE: LazyLock<Regex> =
    LazyLock::new(|| re(r#"\breadlink\s*\(\s*"/proc/self/exe"\s*,\s*"#));
static ZEROMEM: LazyLock<Regex> =
    LazyLock::new(|| re(r"\b(ZeroMemory|SecureZeroMemory)\s*\(([^,()]+),"));
static COPYMEM: LazyLock<Regex> = LazyLock::new(|| re(r"\b(CopyMemory|MoveMemory)\s*\("));
static DEBUGSTR: LazyLock<Regex> = LazyLock::new(|| re(r"\bOutputDebugString[A]?\s*\("));
/// "dir\\file.ext" string literal (escaped backslashes between plain segments).
static BACKSLASH_C: LazyLock<Regex> =
    LazyLock::new(|| re(r#""([A-Za-z0-9_.\-]+(?:\\\\[A-Za-z0-9_.\-]+)+)""#));

fn is_cpp(lang_id: &str) -> bool {
    matches!(lang_id, "cpp" | "objcpp")
}

/// Rewrite a C/C++/Objective-C file. Returns None when nothing changed.
pub fn rewrite_c(rel: &str, lang_id: &str, text: &str, cx: &RewriteCtx) -> Option<Rewritten> {
    let guards = rules::guard_map(text);
    let to_windows = cx.targets.contains(&Os::Windows);
    let cpp = is_cpp(lang_id);
    let shim_path = format!("{}crossfire/cf_platform.h", up_prefix(rel));
    let mut has_shim = text.contains("cf_platform.h");
    let mut need_shim = false;
    let mut notes: Vec<String> = Vec::new();
    let note = |n: String, notes: &mut Vec<String>| {
        if !notes.contains(&n) {
            notes.push(n);
        }
    };
    let mut out: Vec<String> = Vec::with_capacity(text.lines().count() + 8);
    let mut in_block = false;

    for (idx, line) in text.lines().enumerate() {
        let g = guards.get(idx).cloned().flatten();
        let win_only = g
            .as_ref()
            .map(|a| !a.is_empty() && a.iter().all(|o| *o == Os::Windows))
            .unwrap_or(false);
        let posix_only = g
            .as_ref()
            .map(|a| !a.is_empty() && !a.contains(&Os::Windows))
            .unwrap_or(false);
        let trimmed = line.trim_start();

        // Track /* ... */ blocks so we do not rewrite commented-out code.
        if in_block {
            out.push(line.to_string());
            if line.contains("*/") {
                in_block = false;
            }
            continue;
        }
        if trimmed.starts_with("/*") && !line.contains("*/") {
            in_block = true;
            out.push(line.to_string());
            continue;
        }
        if rules::is_comment(lang_id, line) {
            out.push(line.to_string());
            continue;
        }

        // --- #include handling ---------------------------------------------------
        if let Some(c) = INCLUDE.captures(line) {
            let header = c[3].replace('\\', "/");
            let lower = header.to_ascii_lowercase();
            if WIN_HEADERS.contains(&lower.as_str()) && !win_only {
                if !has_shim {
                    out.push(format!("#include \"{shim_path}\""));
                    has_shim = true;
                }
                out.push("#ifdef _WIN32".into());
                // MinGW and cross toolchains ship lower-case header names.
                out.push(format!("{}{}{}{}{}", &c[1], &c[2], lower, &c[4], &c[5]));
                out.push("#endif".into());
                note(
                    format!("Wrapped <{header}> in #ifdef _WIN32 and included cf_platform.h"),
                    &mut notes,
                );
                continue;
            }
            if to_windows && POSIX_SHIMMED.contains(&lower.as_str()) && c[2].eq("<") && !posix_only
            {
                if !has_shim {
                    out.push(format!("#include \"{shim_path}\""));
                    has_shim = true;
                }
                out.push("#ifndef _WIN32".into());
                out.push(line.to_string());
                out.push("#endif".into());
                note(
                    format!("Wrapped <{header}> in #ifndef _WIN32 (cf_platform.h covers MSVC)"),
                    &mut notes,
                );
                continue;
            }
            if c[2].eq("\"") {
                let mut fixed = header.clone();
                if cx.fix_include_case {
                    if let Some(f) = fix_include_case(rel, &header, cx) {
                        note(
                            format!("Fixed include case: \"{}\" → \"{f}\"", &c[3]),
                            &mut notes,
                        );
                        fixed = f;
                    }
                }
                if fixed != c[3] {
                    if c[3].contains('\\') {
                        note(
                            format!("Normalized include path separators in \"{}\"", &c[3]),
                            &mut notes,
                        );
                    }
                    out.push(format!("{}\"{}\"{}", &c[1], fixed, &c[5]));
                    continue;
                }
            }
            out.push(line.to_string());
            continue;
        }
        if PRAGMA_MSVC.is_match(line) && !win_only {
            out.push("#ifdef _MSC_VER".into());
            out.push(line.to_string());
            out.push("#endif".into());
            note(
                "Wrapped MSVC-only #pragma in #ifdef _MSC_VER".into(),
                &mut notes,
            );
            continue;
        }
        if trimmed.starts_with('#') {
            out.push(line.to_string());
            continue;
        }

        // --- token rewrites --------------------------------------------------------
        let mut l = line.to_string();
        let before = l.clone();
        if !win_only {
            if DECLSPEC.is_match(&l) {
                l = DECLSPEC
                    .replace_all(&l, |c: &Captures| match &c[1] {
                        "dllexport" => "CF_EXPORT".to_string(),
                        "dllimport" => "CF_IMPORT".to_string(),
                        "noinline" => "CF_NOINLINE".to_string(),
                        "noreturn" => "CF_NORETURN".to_string(),
                        "thread" => "CF_THREAD_LOCAL".to_string(),
                        _ => format!("CF_ALIGN({})", c.get(2).map(|m| m.as_str()).unwrap_or("16")),
                    })
                    .into_owned();
                note("__declspec(...) → CF_* attribute macros".into(), &mut notes);
            }
            if FORCEINLINE.is_match(&l) {
                l = FORCEINLINE.replace_all(&l, "CF_FORCEINLINE").into_owned();
                note("__forceinline → CF_FORCEINLINE".into(), &mut notes);
            }
            if SLEEP.is_match(&l) {
                let to = if cpp {
                    "${1}cf::sleep_ms("
                } else {
                    "${1}cf_sleep_ms("
                };
                l = SLEEP.replace_all(&l, to).into_owned();
                note(
                    format!(
                        "Sleep(ms) → {}",
                        if cpp { "cf::sleep_ms" } else { "cf_sleep_ms" }
                    ),
                    &mut notes,
                );
            }
            if TICKS.is_match(&l) {
                let to = if cpp {
                    "${1}cf::ticks_ms()"
                } else {
                    "${1}cf_ticks_ms()"
                };
                l = TICKS.replace_all(&l, to).into_owned();
                note(
                    format!(
                        "GetTickCount() → {}",
                        if cpp {
                            "cf::ticks_ms()"
                        } else {
                            "cf_ticks_ms()"
                        }
                    ),
                    &mut notes,
                );
            }
            if STRICMP.is_match(&l) {
                l = STRICMP
                    .replace_all(&l, |c: &Captures| {
                        // C names even in C++: `cf::stricmp` would be mangled by the
                        // compat macro `#define stricmp cf_stricmp`.
                        let f = if c[2].contains('n') {
                            "cf_strnicmp"
                        } else {
                            "cf_stricmp"
                        };
                        format!("{}{f}(", &c[1])
                    })
                    .into_owned();
                note(
                    "_stricmp/_strnicmp → cf_stricmp/cf_strnicmp (strcasecmp on POSIX)".into(),
                    &mut notes,
                );
            }
            if SNPRINTF.is_match(&l) {
                l = SNPRINTF.replace_all(&l, "${1}${2}(").into_owned();
                note(
                    "_snprintf/_vsnprintf → snprintf/vsnprintf".into(),
                    &mut notes,
                );
            }
            if GETENV.is_match(&l) {
                l = GETENV
                    .replace_all(&l, |c: &Captures| match &c[2] {
                        "APPDATA" | "LOCALAPPDATA" => "cf_user_data_dir()".to_string(),
                        "USERPROFILE" => "cf_home_dir()".to_string(),
                        _ => "cf_temp_dir()".to_string(),
                    })
                    .into_owned();
                note("getenv(\"APPDATA\"/\"USERPROFILE\"/\"TEMP\") → cf_user_data_dir()/cf_home_dir()/cf_temp_dir()".into(), &mut notes);
            }
            if BACKSLASH_C.is_match(&l) {
                l = BACKSLASH_C
                    .replace_all(&l, |c: &Captures| {
                        format!("\"{}\"", c[1].replace("\\\\", "/"))
                    })
                    .into_owned();
                note(
                    "Backslash path literals → forward slashes".into(),
                    &mut notes,
                );
            }
            if cx.idiomatic {
                if ZEROMEM.is_match(&l) {
                    l = ZEROMEM.replace_all(&l, "memset(${2}, 0,").into_owned();
                    note("ZeroMemory → memset".into(), &mut notes);
                }
                if COPYMEM.is_match(&l) {
                    l = COPYMEM
                        .replace_all(&l, |c: &Captures| {
                            if &c[1] == "CopyMemory" {
                                "memcpy("
                            } else {
                                "memmove("
                            }
                            .to_string()
                        })
                        .into_owned();
                    note("CopyMemory/MoveMemory → memcpy/memmove".into(), &mut notes);
                }
                if DEBUGSTR.is_match(&l) {
                    l = DEBUGSTR.replace_all(&l, "cf_debug_log(").into_owned();
                    note("OutputDebugString → cf_debug_log".into(), &mut notes);
                }
            }
        }
        if to_windows && !posix_only {
            if STRCASECMP.is_match(&l) {
                l = STRCASECMP
                    .replace_all(&l, |c: &Captures| {
                        let f = if &c[2] == "strncasecmp" {
                            "cf_strnicmp"
                        } else {
                            "cf_stricmp"
                        };
                        format!("{}{f}(", &c[1])
                    })
                    .into_owned();
                note(
                    "strcasecmp/strncasecmp → cf_stricmp/cf_strnicmp (_stricmp on Windows)".into(),
                    &mut notes,
                );
            }
            if USLEEP.is_match(&l) {
                l = USLEEP
                    .replace_all(
                        &l,
                        if cpp {
                            "${1}cf::sleep_us("
                        } else {
                            "${1}cf_sleep_us("
                        },
                    )
                    .into_owned();
                note("usleep → cf_sleep_us".into(), &mut notes);
            }
        }
        if PROC_EXE.is_match(&l)
            && g.as_ref()
                .map(|a| a.len() > 1 || !a.contains(&Os::Linux))
                .unwrap_or(true)
        {
            l = PROC_EXE
                .replace_all(&l, "cf_executable_path_into(")
                .into_owned();
            note("readlink(\"/proc/self/exe\") → cf_executable_path_into() (sysctl on FreeBSD, _NSGetExecutablePath on macOS)".into(), &mut notes);
        }
        if l != before && (l.contains("cf_") || l.contains("cf::") || l.contains("CF_")) {
            need_shim = true;
        }
        out.push(l);
    }

    if need_shim && !has_shim {
        let pos = shim_insert_pos(&out);
        out.insert(pos, format!("#include \"{shim_path}\""));
        notes.push("Included crossfire/cf_platform.h".into());
    }
    let mut new = out.join("\n");
    if text.ends_with('\n') {
        new.push('\n');
    }
    if new == text {
        None
    } else {
        Some(Rewritten { text: new, notes })
    }
}

/// Where to insert the shim include: before the first top-level #include
/// (outside any #if, an include guard counts as top level), else after an
/// include guard / #pragma once, else at the top.
fn shim_insert_pos(lines: &[String]) -> usize {
    static IF: LazyLock<Regex> = LazyLock::new(|| re(r"^\s*#\s*if(n?def)?\b"));
    static ENDIF: LazyLock<Regex> = LazyLock::new(|| re(r"^\s*#\s*endif\b"));
    // Detect a classic include guard: first directive #ifndef X, next #define X.
    let directives: Vec<(usize, &String)> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.trim_start().starts_with('#'))
        .collect();
    let mut guard_end = None;
    if let (Some((_, d0)), Some((i1, d1))) = (directives.first(), directives.get(1)) {
        let t0 = d0.trim_start();
        if t0.starts_with("#ifndef") && d1.trim_start().starts_with("#define") {
            let name = t0.trim_start_matches("#ifndef").trim();
            if d1.contains(name) {
                guard_end = Some(*i1 + 1);
            }
        }
    }
    let base = usize::from(guard_end.is_some());
    let mut depth = 0usize;
    for (i, l) in lines.iter().enumerate() {
        if IF.is_match(l) {
            depth += 1;
            continue;
        }
        if ENDIF.is_match(l) {
            depth = depth.saturating_sub(1);
            continue;
        }
        if depth == base && INCLUDE.is_match(l) {
            return i;
        }
    }
    if let Some(g) = guard_end {
        return g;
    }
    if let Some(i) = lines
        .iter()
        .take(40)
        .position(|l| l.trim_start().starts_with("#pragma once"))
    {
        return i + 1;
    }
    0
}

fn norm_join(dir: &str, inc: &str) -> Option<String> {
    let mut parts: Vec<&str> = if dir.is_empty() {
        vec![]
    } else {
        dir.split('/').collect()
    };
    for seg in inc.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            s => parts.push(s),
        }
    }
    Some(parts.join("/"))
}

/// If `inc` (as written in `rel`) only resolves case-insensitively, return the
/// correctly-cased spelling (same number of path segments).
fn fix_include_case(rel: &str, inc: &str, cx: &RewriteCtx) -> Option<String> {
    let dir = rel.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
    let mut bases: Vec<&str> = vec![dir, ""];
    bases.extend(cx.include_dirs.iter().map(String::as_str));
    for base in bases {
        let Some(joined) = norm_join(base, inc) else {
            continue;
        };
        if cx.files_lower.values().any(|v| *v == joined) {
            return None; // exact match exists
        }
        if let Some(actual) = cx.files_lower.get(&joined.to_ascii_lowercase()) {
            let n = inc.split('/').count();
            let segs: Vec<&str> = actual.split('/').collect();
            if segs.len() < n {
                return None;
            }
            // Keep any leading "../" segments from the original spelling.
            let orig: Vec<&str> = inc.split('/').collect();
            let tail = &segs[segs.len() - n..];
            let fixed: Vec<String> = orig
                .iter()
                .zip(tail.iter())
                .map(|(o, t)| {
                    if *o == ".." || *o == "." {
                        o.to_string()
                    } else {
                        t.to_string()
                    }
                })
                .collect();
            let f = fixed.join("/");
            return (f != inc).then_some(f);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Python
// ---------------------------------------------------------------------------

static PY_IMPORT_WINREG: LazyLock<Regex> =
    LazyLock::new(|| re(r"^(\s*)import\s+(_?winreg)(\s+as\s+(\w+))?\s*$"));
static PY_FROM_WINREG: LazyLock<Regex> =
    LazyLock::new(|| re(r"^(\s*)from\s+_?winreg\s+import\s+(.+)$"));
static PY_IMPORT_MSVCRT: LazyLock<Regex> =
    LazyLock::new(|| re(r"^(\s*)import\s+msvcrt(\s+as\s+(\w+))?\s*$"));
static PY_STARTFILE: LazyLock<Regex> = LazyLock::new(|| re(r"\bos\.startfile\s*\("));
static PY_CLS: LazyLock<Regex> = LazyLock::new(|| re(r#"\bos\.system\s*\(\s*['"]cls['"]\s*\)"#));
static PY_ENV: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r#"\bos\.(?:environ\s*\[\s*['"](APPDATA|LOCALAPPDATA|USERPROFILE|TEMP|TMP)['"]\s*\]|environ\.get\s*\(\s*['"](APPDATA|LOCALAPPDATA|USERPROFILE|TEMP|TMP)['"]\s*(?:,[^()]*)?\)|getenv\s*\(\s*['"](APPDATA|LOCALAPPDATA|USERPROFILE|TEMP|TMP)['"]\s*(?:,[^()]*)?\))"#,
    )
});
static PY_RAW_BACKSLASH: LazyLock<Regex> =
    LazyLock::new(|| re(r#"\b[rR](["'])([A-Za-z0-9_.\-]+(?:\\[A-Za-z0-9_.\-]+)+)(["'])"#));
static PY_BACKSLASH: LazyLock<Regex> =
    LazyLock::new(|| re(r#"(^|[^\w])(["'])([A-Za-z0-9_.\-]+(?:\\\\[A-Za-z0-9_.\-]+)+)(["'])"#));
static PY_TOP_IMPORT: LazyLock<Regex> =
    LazyLock::new(|| re(r"^(import\s+\w|from\s+[\w.]+\s+import\s)"));

pub fn rewrite_python(rel: &str, text: &str) -> Option<Rewritten> {
    let mut notes: Vec<String> = Vec::new();
    let mut need_platform = false;
    let mut uses_crossfire = false;
    let mut out: Vec<String> = Vec::new();
    for line in text.lines() {
        if line.trim_start().starts_with('#') {
            out.push(line.to_string());
            continue;
        }
        let mut l = line.to_string();
        if let Some(c) = PY_IMPORT_WINREG.captures(&l) {
            let alias = c
                .get(4)
                .map(|m| m.as_str().to_string())
                .unwrap_or_else(|| c[2].to_string());
            l = format!("{}from crossfire import cf_winreg as {alias}", &c[1]);
            notes.push("import winreg → crossfire.cf_winreg (JSON-backed off Windows)".into());
            uses_crossfire = true;
        } else if let Some(c) = PY_FROM_WINREG.captures(&l) {
            l = format!("{}from crossfire.cf_winreg import {}", &c[1], &c[2]);
            notes.push("from winreg import … → crossfire.cf_winreg".into());
            uses_crossfire = true;
        } else if let Some(c) = PY_IMPORT_MSVCRT.captures(&l) {
            let alias = c.get(3).map(|m| m.as_str()).unwrap_or("msvcrt");
            l = format!("{}from crossfire import cf_msvcrt as {alias}", &c[1]);
            notes.push("import msvcrt → crossfire.cf_msvcrt (termios off Windows)".into());
            uses_crossfire = true;
        }
        if PY_STARTFILE.is_match(&l) {
            l = PY_STARTFILE
                .replace_all(&l, "cf_platform.open_path(")
                .into_owned();
            notes.push("os.startfile → cf_platform.open_path".into());
            need_platform = true;
        }
        if PY_CLS.is_match(&l) {
            l = PY_CLS
                .replace_all(&l, "cf_platform.clear_screen()")
                .into_owned();
            notes.push("os.system('cls') → cf_platform.clear_screen()".into());
            need_platform = true;
        }
        if PY_ENV.is_match(&l) {
            l = PY_ENV
                .replace_all(&l, |c: &Captures| {
                    let var = c
                        .get(1)
                        .or(c.get(2))
                        .or(c.get(3))
                        .map(|m| m.as_str())
                        .unwrap_or("");
                    match var {
                        "APPDATA" => "cf_platform.user_data_dir()",
                        "LOCALAPPDATA" => "cf_platform.user_data_dir(local=True)",
                        "USERPROFILE" => "cf_platform.home_dir()",
                        _ => "cf_platform.temp_dir()",
                    }
                    .to_string()
                })
                .into_owned();
            notes.push("%APPDATA%/%USERPROFILE%/%TEMP% lookups → cf_platform helpers".into());
            need_platform = true;
        }
        if PY_RAW_BACKSLASH.is_match(&l) {
            l = PY_RAW_BACKSLASH
                .replace_all(&l, |c: &Captures| {
                    format!("{}{}{}", &c[1], c[2].replace('\\', "/"), &c[3])
                })
                .into_owned();
            notes.push("Backslash path literals → forward slashes".into());
        }
        if PY_BACKSLASH.is_match(&l) {
            l = PY_BACKSLASH
                .replace_all(&l, |c: &Captures| {
                    format!("{}{}{}{}", &c[1], &c[2], c[3].replace("\\\\", "/"), &c[4])
                })
                .into_owned();
            notes.push("Backslash path literals → forward slashes".into());
        }
        out.push(l);
    }
    if need_platform {
        uses_crossfire = true;
    }
    if uses_crossfire {
        // Insert the bootstrap before the first top-level import (after any
        // docstring / __future__ imports), or at the top.
        let pos = out
            .iter()
            .position(|l| PY_TOP_IMPORT.is_match(l) && !l.starts_with("from __future__"))
            .unwrap_or_else(|| {
                out.iter()
                    .position(|l| !l.starts_with("#!") && !l.starts_with("# -*-"))
                    .unwrap_or(0)
            });
        let mut ins: Vec<String> = Vec::new();
        let depth = rel.matches('/').count();
        if depth > 0 {
            ins.push("import os as _cf_os, sys as _cf_sys  # crossfire: make the crossfire package importable".into());
            ins.push(format!(
                "_cf_sys.path.insert(0, _cf_os.path.join(_cf_os.path.dirname(_cf_os.path.abspath(__file__)), {:?}))",
                "../".repeat(depth).trim_end_matches('/')
            ));
        }
        if need_platform {
            ins.push("from crossfire import cf_platform".into());
        }
        for (i, l) in ins.into_iter().enumerate() {
            out.insert(pos + i, l);
        }
    }
    notes.dedup();
    let mut new = out.join("\n");
    if text.ends_with('\n') {
        new.push('\n');
    }
    (new != text).then_some(Rewritten { text: new, notes })
}

// ---------------------------------------------------------------------------
// C#
// ---------------------------------------------------------------------------

static CS_VERBATIM: LazyLock<Regex> =
    LazyLock::new(|| re(r#"@"([A-Za-z0-9_.\-]+(?:\\[A-Za-z0-9_.\-]+)+)""#));
static CS_ESCAPED: LazyLock<Regex> =
    LazyLock::new(|| re(r#"(^|[^@\w])"([A-Za-z0-9_.\-]+(?:\\\\[A-Za-z0-9_.\-]+)+)""#));
static CS_ENV: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r#"\bEnvironment\.GetEnvironmentVariable\s*\(\s*"(APPDATA|LOCALAPPDATA|USERPROFILE|TEMP|TMP)"\s*\)"#,
    )
});
static CS_EXPLORER: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r#"\b(?:System\.Diagnostics\.)?Process\.Start\s*\(\s*"explorer(?:\.exe)?"\s*,\s*([^;]+?)\)\s*;"#,
    )
});

pub fn rewrite_csharp(text: &str) -> Option<Rewritten> {
    let mut notes = Vec::new();
    let mut out = Vec::new();
    for line in text.lines() {
        if line.trim_start().starts_with("//") {
            out.push(line.to_string());
            continue;
        }
        let mut l = line.to_string();
        if CS_VERBATIM.is_match(&l) {
            l = CS_VERBATIM
                .replace_all(&l, |c: &Captures| {
                    format!("\"{}\"", c[1].replace('\\', "/"))
                })
                .into_owned();
            notes.push("Backslash path literals → forward slashes".to_string());
        }
        if CS_ESCAPED.is_match(&l) {
            l = CS_ESCAPED
                .replace_all(&l, |c: &Captures| {
                    format!("{}\"{}\"", &c[1], c[2].replace("\\\\", "/"))
                })
                .into_owned();
            notes.push("Backslash path literals → forward slashes".to_string());
        }
        if CS_ENV.is_match(&l) {
            l = CS_ENV
                .replace_all(&l, |c: &Captures| match &c[1] {
                    "APPDATA" | "LOCALAPPDATA" => "Crossfire.CfPlatform.UserDataDir".to_string(),
                    "USERPROFILE" => "Crossfire.CfPlatform.HomeDir".to_string(),
                    _ => "Crossfire.CfPlatform.TempDir".to_string(),
                })
                .into_owned();
            notes.push("Windows env-var paths → Crossfire.CfPlatform".to_string());
        }
        if CS_EXPLORER.is_match(&l) {
            l = CS_EXPLORER
                .replace_all(&l, "Crossfire.CfPlatform.OpenPath(${1});")
                .into_owned();
            notes.push("Process.Start(\"explorer\", …) → CfPlatform.OpenPath".to_string());
        }
        out.push(l);
    }
    notes.dedup();
    let mut new = out.join("\n");
    if text.ends_with('\n') {
        new.push('\n');
    }
    (new != text).then_some(Rewritten { text: new, notes })
}

/// SDK-style .csproj: allow building Windows-targeted projects on macOS/Linux.
pub fn rewrite_csproj(text: &str) -> Option<Rewritten> {
    if !text.contains("-windows") || text.contains("EnableWindowsTargeting") {
        return None;
    }
    let pos = text.find("<PropertyGroup>")? + "<PropertyGroup>".len();
    let new = format!(
        "{}\n    <!-- Crossfire: lets `dotnet build` run on macOS/Linux (the WinForms/WPF UI still runs only on Windows). -->\n    <EnableWindowsTargeting>true</EnableWindowsTargeting>{}",
        &text[..pos],
        &text[pos..]
    );
    Some(Rewritten {
        text: new,
        notes: vec![
            "Added <EnableWindowsTargeting> so the project builds on non-Windows hosts".into(),
        ],
    })
}

/// Shell scripts: portable shebang (bash lives in /usr/local/bin on FreeBSD).
pub fn rewrite_shell(text: &str) -> Option<Rewritten> {
    let first = text.lines().next()?;
    if first.trim() == "#!/bin/bash" || first.starts_with("#!/bin/bash ") {
        let rest = &text[first.len()..];
        let args = first.trim_start_matches("#!/bin/bash");
        return Some(Rewritten {
            text: format!("#!/usr/bin/env bash{args}{rest}"),
            notes: vec![
                "#!/bin/bash → #!/usr/bin/env bash (FreeBSD installs bash in /usr/local/bin)"
                    .into(),
            ],
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cx<'a>(
        targets: &'a [Os],
        files: &'a HashMap<String, String>,
        dirs: &'a [String],
    ) -> RewriteCtx<'a> {
        RewriteCtx {
            targets,
            idiomatic: false,
            fix_include_case: true,
            files_lower: files,
            include_dirs: dirs,
        }
    }

    #[test]
    fn c_rewrites_win32_basics() {
        let files = HashMap::new();
        let t = [Os::Linux, Os::Macos];
        let src = "#include <Windows.h>\n#include <stdio.h>\nint main(){\n  Sleep(16);\n  DWORD t = GetTickCount();\n  if (_stricmp(a, b) == 0) {}\n  FILE* f = fopen(\"data\\\\levels\\\\one.txt\", \"r\");\n  char* p = getenv(\"APPDATA\");\n}\n";
        let r = rewrite_c("src/main.c", "c", src, &cx(&t, &files, &[])).unwrap();
        assert!(
            r.text.contains("#include \"../crossfire/cf_platform.h\""),
            "{}",
            r.text
        );
        assert!(
            r.text
                .contains("#ifdef _WIN32\n#include <windows.h>\n#endif"),
            "{}",
            r.text
        );
        assert!(r.text.contains("cf_sleep_ms(16)"));
        assert!(r.text.contains("cf_ticks_ms()"));
        assert!(r.text.contains("cf_stricmp(a, b)"));
        assert!(r.text.contains("\"data/levels/one.txt\""));
        assert!(r.text.contains("cf_user_data_dir()"));
    }

    #[test]
    fn cpp_uses_namespace_and_skips_guarded_code() {
        let files = HashMap::new();
        let t = [Os::Linux];
        let src = "#ifdef _WIN32\n#include <windows.h>\nvoid w(){ Sleep(1); }\n#endif\nvoid f(){ Sleep(2); obj.Sleep(3); }\n";
        let r = rewrite_c("a.cpp", "cpp", src, &cx(&t, &files, &[])).unwrap();
        assert!(
            r.text.contains("void w(){ Sleep(1); }"),
            "guarded code untouched: {}",
            r.text
        );
        assert!(r.text.contains("cf::sleep_ms(2)"));
        assert!(r.text.contains("obj.Sleep(3)"));
        assert!(
            r.text.starts_with(
                "#include \"crossfire/cf_platform.h\"\n#ifdef _WIN32\n#include <windows.h>"
            ),
            "{}",
            r.text
        );
    }

    #[test]
    fn fixes_include_case() {
        let mut files = HashMap::new();
        files.insert("src/renderer.h".to_string(), "src/Renderer.h".to_string());
        let t = [Os::Linux];
        let r = rewrite_c(
            "src/main.cpp",
            "cpp",
            "#include \"renderer.h\"\n",
            &cx(&t, &files, &[]),
        )
        .unwrap();
        assert_eq!(r.text, "#include \"Renderer.h\"\n");
    }

    #[test]
    fn python_winreg_and_startfile() {
        let src = "import os\nimport winreg\nos.startfile(path)\nd = os.environ['APPDATA']\np = r\"assets\\sounds\\a.wav\"\n";
        let r = rewrite_python("tool.py", src).unwrap();
        assert!(r.text.contains("from crossfire import cf_winreg as winreg"));
        assert!(r.text.contains("cf_platform.open_path(path)"));
        assert!(r.text.contains("d = cf_platform.user_data_dir()"));
        assert!(r.text.contains("\"assets/sounds/a.wav\""));
        assert!(
            r.text
                .starts_with("from crossfire import cf_platform\nimport os"),
            "{}",
            r.text
        );
    }

    #[test]
    fn csharp_paths() {
        let r = rewrite_csharp("var p = @\"Assets\\Icons\\app.png\";\nvar d = Environment.GetEnvironmentVariable(\"APPDATA\");\n").unwrap();
        assert!(r.text.contains("\"Assets/Icons/app.png\""));
        assert!(r.text.contains("Crossfire.CfPlatform.UserDataDir"));
    }
}
