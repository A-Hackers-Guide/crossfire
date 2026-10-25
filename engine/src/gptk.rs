//! Run a Windows .exe through Apple's Game Porting Toolkit (or plain Wine).

use crate::model::{Level, ProgressEvent, Stage};
use crate::toolchain::gptk_wine;
use crate::util::{run_streaming, which};
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

/// Default run limit; override with CROSSFIRE_GPTK_TIMEOUT (seconds, 0 = none).
const DEFAULT_TIMEOUT_SECS: u64 = 600;

/// Dedicated prefix: ~/Library/Application Support/Crossfire/gptk-prefix.
pub fn prefix_dir() -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"));
    home.join("Library/Application Support/Crossfire/gptk-prefix")
}

pub fn test_with_gptk(exe: &Path, on: &dyn Fn(ProgressEvent)) -> Result<String> {
    if !exe.is_file() {
        bail!("{} does not exist", exe.display());
    }
    let is_pe = std::fs::read(exe).map(|b| b.starts_with(b"MZ")).unwrap_or(false);
    if !is_pe {
        bail!("{} is not a Windows PE executable", exe.display());
    }
    let (wine, flavor) = match gptk_wine() {
        Some(w) if cfg!(target_os = "macos") => (w, "Game Porting Toolkit"),
        _ => match which("wine64").or_else(|| which("wine")) {
            Some(w) => (w, "Wine"),
            None => bail!(if cfg!(target_os = "macos") {
                "Game Porting Toolkit not found (expected /Applications/Game Porting Toolkit.app) and no wine in PATH"
            } else {
                "Game Porting Toolkit is macOS-only and no wine was found in PATH"
            }),
        },
    };
    let prefix = prefix_dir();
    let fresh = !prefix.join("system.reg").exists();
    std::fs::create_dir_all(&prefix).with_context(|| format!("creating {}", prefix.display()))?;
    on(ProgressEvent::new(
        Stage::Test,
        Level::Info,
        0.05,
        format!("Running {} with {flavor} ({}){}", exe.display(), wine.display(), if fresh { "; initializing a new prefix" } else { "" }),
    ));

    let timeout = std::env::var("CROSSFIRE_GPTK_TIMEOUT")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(DEFAULT_TIMEOUT_SECS);
    let mut cmd = Command::new(&wine);
    cmd.arg(exe)
        .current_dir(exe.parent().unwrap_or(Path::new(".")))
        .env("WINEPREFIX", &prefix)
        .env("WINEDEBUG", std::env::var("WINEDEBUG").unwrap_or_else(|_| "fixme-all,err+all".into()))
        .env("WINEESYNC", "1")
        .env("WINEMSYNC", "1");
    if flavor == "Game Porting Toolkit" {
        // Metal HUD shows FPS / GPU time, handy for a first look.
        cmd.env("MTL_HUD_ENABLED", std::env::var("MTL_HUD_ENABLED").unwrap_or_else(|_| "1".into()));
        cmd.env("D3DM_SUPPORT_DXR", "1");
    }
    let started = Instant::now();
    let out = run_streaming(cmd, Stage::Test, on, (timeout > 0).then(|| Duration::from_secs(timeout)));
    let secs = started.elapsed().as_secs_f64();
    let summary = summarize(&out.log, out.code, out.timed_out, secs, flavor, &prefix);
    let level = if out.success || out.timed_out { Level::Success } else { Level::Warn };
    on(ProgressEvent::new(Stage::Test, level, 1.0, summary.lines().next().unwrap_or("").to_string()));
    Ok(summary)
}

/// Condense a wine log into a short human summary.
pub fn summarize(log: &str, code: Option<i32>, timed_out: bool, secs: f64, flavor: &str, prefix: &Path) -> String {
    let lines: Vec<&str> = log.lines().collect();
    let errs: Vec<&str> = lines.iter().copied().filter(|l| l.contains("err:")).collect();
    let mut missing_dlls: Vec<String> = Vec::new();
    for l in &lines {
        // e.g. err:module:import_dll Library d3dx9_43.dll (which is needed by ...) not found
        if let Some(i) = l.find("Library ") {
            if l.contains("not found") {
                let name: String = l[i + 8..].split_whitespace().next().unwrap_or("").to_string();
                if !name.is_empty() && !missing_dlls.contains(&name) {
                    missing_dlls.push(name);
                }
            }
        }
    }
    let d3d = lines.iter().any(|l| l.contains("D3DM") || l.contains("d3d") || l.contains("D3D"));
    let mut s = String::new();
    let status = if timed_out {
        format!("still running after {secs:.0}s (stopped by timeout)")
    } else {
        match code {
            Some(0) => format!("exited normally after {secs:.1}s"),
            Some(c) => format!("exited with code {c} after {secs:.1}s"),
            None => format!("terminated by a signal after {secs:.1}s"),
        }
    };
    s.push_str(&format!("{flavor}: {status}.\n"));
    s.push_str(&format!("Output: {} lines, {} wine error lines.\n", lines.len(), errs.len()));
    if !missing_dlls.is_empty() {
        s.push_str(&format!("Missing DLLs: {} (install redistributables into the prefix with winetricks).\n", missing_dlls.join(", ")));
    }
    if d3d {
        s.push_str("Direct3D activity detected (translated by D3DMetal/wined3d).\n");
    }
    for e in errs.iter().take(5) {
        s.push_str(&format!("  {e}\n"));
    }
    s.push_str(&format!("Prefix: {}\n", prefix.display()));
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_lists_missing_dlls() {
        let log = "err:module:import_dll Library d3dx9_43.dll (which is needed by L\"C:\\\\game.exe\") not found\nhello\n";
        let s = summarize(log, Some(1), false, 2.0, "Wine", Path::new("/tmp/p"));
        assert!(s.contains("d3dx9_43.dll"));
        assert!(s.contains("exited with code 1"));
    }

    #[test]
    fn rejects_non_pe() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("x.exe");
        std::fs::write(&f, b"not a pe").unwrap();
        assert!(test_with_gptk(&f, &|_| {}).is_err());
    }
}
