//! Process helpers: executable lookup, short version probes and streaming runs.

use crate::model::{Level, ProgressEvent, Stage};
use regex::Regex;
use std::io::{BufRead, BufReader};
use std::sync::LazyLock;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// Directories searched in addition to PATH. GUI apps on macOS start with a
/// minimal PATH, so Homebrew/cargo/dotnet locations are added explicitly.
fn extra_dirs() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = ["/opt/homebrew/bin", "/usr/local/bin", "/opt/local/bin", "/usr/bin", "/bin", "/usr/local/share/dotnet", "/usr/local/go/bin"]
        .iter()
        .map(PathBuf::from)
        .collect();
    if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
        let h = PathBuf::from(home);
        for d in [".cargo/bin", ".dotnet", "go/bin", ".local/bin", ".zig", "bin"] {
            v.push(h.join(d));
        }
    }
    v.push(PathBuf::from("/opt/homebrew/opt/rustup/bin"));
    v
}

/// Find an executable by name in PATH (plus well-known dirs).
pub fn which(name: &str) -> Option<PathBuf> {
    let p = Path::new(name);
    if p.is_absolute() {
        return is_exe(p).then(|| p.to_path_buf());
    }
    let exts: Vec<String> = if cfg!(windows) {
        std::env::var("PATHEXT").unwrap_or(".EXE;.CMD;.BAT".into()).split(';').map(|s| s.to_ascii_lowercase()).collect()
    } else {
        vec![String::new()]
    };
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    dirs.extend(extra_dirs());
    for d in dirs {
        for e in &exts {
            let cand = d.join(format!("{name}{e}"));
            if is_exe(&cand) {
                return Some(cand);
            }
        }
    }
    None
}

fn is_exe(p: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        p.is_file() && p.metadata().map(|m| m.permissions().mode() & 0o111 != 0).unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        p.is_file()
    }
}

/// PATH value including the extra directories, for child processes.
pub fn augmented_path() -> std::ffi::OsString {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    for d in extra_dirs() {
        if !dirs.contains(&d) {
            dirs.push(d);
        }
    }
    std::env::join_paths(dirs).unwrap_or_default()
}

/// Run a command with a timeout and return combined stdout+stderr (None on
/// spawn failure or timeout).
pub fn capture(cmd: &Path, args: &[&str], timeout: Duration) -> Option<(bool, String)> {
    let mut child = Command::new(cmd)
        .args(args)
        .env("PATH", augmented_path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if start.elapsed() > timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(_) => return None,
        }
    }
    let out = child.wait_with_output().ok()?;
    let mut s = String::from_utf8_lossy(&out.stdout).to_string();
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    Some((out.status.success(), s))
}

/// First meaningful line of a version probe, trimmed.
pub fn version_line(cmd: &Path, args: &[&str]) -> Option<String> {
    let (_, out) = capture(cmd, args, Duration::from_secs(8))?;
    out.lines().map(str::trim).find(|l| !l.is_empty()).map(|l| l.chars().take(120).collect())
}

/// Result of a streamed command.
pub struct RunOutcome {
    pub success: bool,
    pub code: Option<i32>,
    pub log: String,
    pub timed_out: bool,
}

const LOG_CAP: usize = 256 * 1024;

/// Severity of a build-output line: compiler/CMake diagnostics, not any line
/// that merely contains "error" (e.g. `SDL_error.c`).
fn classify_line(line: &str, is_stderr: bool) -> Level {
    static ERR: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)(\berror(\[\w+\])?:|\bfatal error\b|^CMake Error|^\s*FAILED:|\*\*\* .*Error \d+|undefined (symbol|reference)|^err:)").unwrap()
    });
    static WARN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)(\bwarning(\[\w+\])?:|^CMake Warning|^warn:)").unwrap());
    if ERR.is_match(line) {
        Level::Error
    } else if WARN.is_match(line) {
        Level::Warn
    } else {
        let _ = is_stderr;
        Level::Info
    }
}

/// Run a command, streaming each output line as a ProgressEvent. The log
/// keeps the last LOG_CAP bytes.
pub fn run_streaming(
    mut cmd: Command,
    stage: Stage,
    on: &dyn Fn(ProgressEvent),
    timeout: Option<Duration>,
) -> RunOutcome {
    let display = format!("{:?}", cmd).replace('"', "");
    on(ProgressEvent::new(stage, Level::Info, -1.0, format!("$ {display}")));
    cmd.env("PATH", augmented_path()).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            let msg = format!("failed to start {display}: {e}");
            on(ProgressEvent::new(stage, Level::Error, -1.0, msg.clone()));
            return RunOutcome { success: false, code: None, log: msg, timed_out: false };
        }
    };
    let (tx, rx) = mpsc::channel::<(bool, String)>();
    let mut readers = Vec::new();
    if let Some(out) = child.stdout.take() {
        let tx = tx.clone();
        readers.push(std::thread::spawn(move || {
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                let _ = tx.send((false, line));
            }
        }));
    }
    if let Some(err) = child.stderr.take() {
        let tx = tx.clone();
        readers.push(std::thread::spawn(move || {
            for line in BufReader::new(err).lines().map_while(Result::ok) {
                let _ = tx.send((true, line));
            }
        }));
    }
    drop(tx);
    let start = Instant::now();
    let mut log = String::new();
    let mut timed_out = false;
    loop {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok((is_err, line)) => {
                let level = classify_line(&line, is_err);
                on(ProgressEvent::new(stage, level, -1.0, line.clone()));
                log.push_str(&line);
                log.push('\n');
                if log.len() > LOG_CAP * 2 {
                    let cut = log.len() - LOG_CAP;
                    let cut = (cut..log.len()).find(|i| log.is_char_boundary(*i)).unwrap_or(cut);
                    log = format!("[... earlier output truncated ...]\n{}", &log[cut..]);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if let Some(t) = timeout {
                    if start.elapsed() > t {
                        let _ = child.kill();
                        timed_out = true;
                        break;
                    }
                }
                if let Ok(Some(_)) = child.try_wait() {
                    // Drain whatever remains, then stop.
                    while let Ok((_, line)) = rx.recv_timeout(Duration::from_millis(100)) {
                        on(ProgressEvent::new(stage, Level::Info, -1.0, line.clone()));
                        log.push_str(&line);
                        log.push('\n');
                    }
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    let status = child.wait().ok();
    for r in readers {
        let _ = r.join();
    }
    RunOutcome {
        success: !timed_out && status.map(|s| s.success()).unwrap_or(false),
        code: status.and_then(|s| s.code()),
        log,
        timed_out,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_build_lines() {
        assert_eq!(classify_line("[ 10%] Building C object SDL_error.c.o", false), Level::Info);
        assert_eq!(classify_line("src/a.c:3:1: error: expected ';'", true), Level::Error);
        assert_eq!(classify_line("CMake Error at CMakeLists.txt:4", true), Level::Error);
        assert_eq!(classify_line("a.c:1: warning: unused", true), Level::Warn);
    }
}
