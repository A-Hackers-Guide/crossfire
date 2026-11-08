//! Build a (converted) project for each requested target OS, picking the best
//! available path: native host build, cross-compilation (zig / mingw / cargo
//! targets / GOOS / dotnet RIDs), containers, or the generated CI workflow.

use crate::model::*;
use crate::templates::{self, BuildKind, CiPlan};
use crate::toolchain::rustup_targets;
use crate::util::{run_streaming, which};
use anyhow::{bail, Result};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

const WORKFLOW: &str = ".github/workflows/crossfire-build.yml";

fn detect_kind(project: &Path) -> BuildKind {
    let has = |f: &str| project.join(f).is_file();
    let first_with_ext = |ext: &str| -> Option<String> {
        let mut v: Vec<String> = std::fs::read_dir(project)
            .ok()?
            .flatten()
            .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some(ext))
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        v.sort();
        v.into_iter().next()
    };
    if ["project.godot"].iter().any(|f| has(f)) {
        return BuildKind::Engine("Godot".into());
    }
    if project.join("ProjectSettings").is_dir() && project.join("Assets").is_dir() {
        return BuildKind::Engine("Unity".into());
    }
    if first_with_ext("uproject").is_some() {
        return BuildKind::Engine("Unreal".into());
    }
    if has("CMakeLists.txt") {
        return BuildKind::CMake;
    }
    if has("meson.build") {
        return BuildKind::Meson;
    }
    if has("Cargo.toml") {
        return BuildKind::Cargo;
    }
    if let Some(p) = first_with_ext("csproj")
        .or_else(|| first_with_ext("fsproj"))
        .or_else(|| first_with_ext("sln"))
    {
        return BuildKind::DotNet { project: p };
    }
    if has("go.mod") {
        return BuildKind::Go;
    }
    if has("package.json") {
        return BuildKind::Node;
    }
    if has("Makefile") || has("GNUmakefile") {
        return BuildKind::Make;
    }
    if has("main.py") || first_with_ext("py").is_some() {
        let entry = if has("main.py") {
            "main.py".to_string()
        } else {
            first_with_ext("py").unwrap_or_default()
        };
        return BuildKind::Python { entry };
    }
    BuildKind::Other
}

fn host_arch() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "x86_64"
    }
}

/// Make sure a CI workflow exists so "ci" results have somewhere to point.
fn ensure_workflow(project: &Path, kind: &BuildKind, targets: &[Os]) -> Option<String> {
    let p = project.join(WORKFLOW);
    if !p.is_file() {
        let name = project
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "app".into());
        let wf = templates::workflow(&CiPlan {
            name: &name,
            kind,
            targets,
            apt: vec![],
            pkg: vec![],
            sdl3: false,
            packages: &[],
        });
        std::fs::create_dir_all(p.parent()?).ok()?;
        std::fs::write(&p, wf).ok()?;
    }
    Some(p.to_string_lossy().to_string())
}

pub fn build(req: &BuildRequest, on: &dyn Fn(ProgressEvent)) -> Result<BuildResult> {
    let project = PathBuf::from(&req.project);
    if !project.is_dir() {
        bail!("project {} is not a directory", project.display());
    }
    let project = project.canonicalize()?;
    let targets: Vec<Os> = if req.targets.is_empty() {
        vec![Os::host()]
    } else {
        req.targets.clone()
    };
    let kind = detect_kind(&project);
    on(ProgressEvent::new(
        Stage::Build,
        Level::Info,
        0.0,
        format!(
            "Building {} ({kind:?}) for {}",
            project.display(),
            targets
                .iter()
                .map(|o| o.label())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    ));
    let ci = ensure_workflow(&project, &kind, &targets);
    let mut results = Vec::new();
    for (i, t) in targets.iter().enumerate() {
        on(ProgressEvent::new(
            Stage::Build,
            Level::Info,
            i as f64 / targets.len() as f64,
            format!("Target {}", t.label()),
        ));
        let started = SystemTime::now();
        let mut r = build_target(&project, &kind, *t, req.release, on);
        if r.status == BuildStatus::Success {
            if r.artifacts.is_empty() {
                r.artifacts = find_artifacts(
                    &project.join("build").join(format!("crossfire-{}", t.id())),
                    started,
                )
                .into_iter()
                .chain(find_artifacts(&project, started))
                .collect();
                r.artifacts.sort();
                r.artifacts.dedup();
            }
            if req.package {
                package(&project, *t, &mut r, on);
            }
        }
        if r.status == BuildStatus::Ci && r.reason.is_some() {
            if let Some(ci) = &ci {
                let reason = r.reason.take().unwrap_or_default();
                r.reason = Some(format!("{reason} — built by the CI workflow {ci}"));
            }
        }
        let level = match r.status {
            BuildStatus::Success => Level::Success,
            BuildStatus::Failed => Level::Error,
            _ => Level::Warn,
        };
        on(ProgressEvent::new(
            Stage::Build,
            level,
            (i + 1) as f64 / targets.len() as f64,
            format!(
                "{}: {:?}{}",
                t.label(),
                r.status,
                r.reason
                    .as_ref()
                    .map(|x| format!(" ({x})"))
                    .unwrap_or_default()
            ),
        ));
        results.push(r);
    }
    Ok(BuildResult {
        results,
        ci_workflow: ci,
    })
}

fn result(
    target: Os,
    status: BuildStatus,
    log: String,
    reason: Option<String>,
) -> BuildTargetResult {
    BuildTargetResult {
        target,
        status,
        artifacts: vec![],
        log,
        reason,
    }
}

fn ci(target: Os, reason: impl Into<String>) -> BuildTargetResult {
    result(target, BuildStatus::Ci, String::new(), Some(reason.into()))
}

/// Run a sequence of commands, stopping at the first failure.
fn run_all(
    target: Os,
    cmds: Vec<Command>,
    on: &dyn Fn(ProgressEvent),
    reason: Option<String>,
) -> BuildTargetResult {
    let mut log = String::new();
    for c in cmds {
        let o = run_streaming(c, Stage::Build, on, None);
        log.push_str(&o.log);
        if !o.success {
            return result(
                target,
                BuildStatus::Failed,
                log,
                Some(format!("command failed (exit {:?})", o.code)),
            );
        }
    }
    result(target, BuildStatus::Success, log, reason)
}

fn build_target(
    project: &Path,
    kind: &BuildKind,
    t: Os,
    release: bool,
    on: &dyn Fn(ProgressEvent),
) -> BuildTargetResult {
    let host = Os::host();
    let cfg = if release { "Release" } else { "Debug" };
    match kind {
        BuildKind::CMake => cmake_target(project, t, cfg, on),
        BuildKind::Meson => {
            if t != host {
                return ci(
                    t,
                    "Meson cross files are not generated; cross builds run in CI",
                );
            }
            let Some(meson) = which("meson") else {
                return ci(t, "meson not installed");
            };
            let bdir = project.join(format!("build/crossfire-{}", t.id()));
            let mut setup = Command::new(&meson);
            setup
                .args(["setup", "--reconfigure"])
                .arg(&bdir)
                .arg(format!(
                    "--buildtype={}",
                    if release { "release" } else { "debug" }
                ))
                .current_dir(project);
            let mut comp = Command::new(&meson);
            comp.arg("compile")
                .arg("-C")
                .arg(&bdir)
                .current_dir(project);
            run_all(t, vec![setup, comp], on, None)
        }
        BuildKind::Make => {
            if t != host {
                return ci(t, "Makefile builds only for the host");
            }
            let make = if host == Os::Freebsd {
                which("gmake").or_else(|| which("make"))
            } else {
                which("make")
            };
            let Some(make) = make else {
                return ci(t, "make not installed");
            };
            let mut c = Command::new(make);
            c.arg("-j4").current_dir(project);
            run_all(t, vec![c], on, None)
        }
        BuildKind::Cargo => cargo_target(project, t, release, on),
        BuildKind::DotNet { project: proj } => {
            let Some(dotnet) = which("dotnet") else {
                return ci(t, ".NET SDK not installed");
            };
            let rid =
                match t {
                    Os::Windows => "win-x64".to_string(),
                    Os::Macos => format!(
                        "osx-{}",
                        if host_arch() == "aarch64" {
                            "arm64"
                        } else {
                            "x64"
                        }
                    ),
                    Os::Linux => "linux-x64".to_string(),
                    Os::Freebsd => return ci(
                        t,
                        "no official .NET runtime identifier for FreeBSD (community builds only)",
                    ),
                };
            let outdir = project.join(format!("build/crossfire-{}", t.id()));
            let mut c = Command::new(dotnet);
            c.arg("publish")
                .arg(proj)
                .args([
                    "-c",
                    cfg,
                    "-r",
                    &rid,
                    "--self-contained",
                    "true",
                    "-p:EnableWindowsTargeting=true",
                    "-o",
                ])
                .arg(&outdir)
                .current_dir(project);
            run_all(t, vec![c], on, None)
        }
        BuildKind::Go => {
            let Some(go) = which("go") else {
                return ci(t, "Go not installed");
            };
            let uses_cgo = dir_contains(project, "go", "import \"C\"");
            if uses_cgo && t != host {
                return ci(
                    t,
                    "package uses cgo; cross-compiling needs a C toolchain per target",
                );
            }
            let (goos, arch) = match t {
                Os::Windows => ("windows", "amd64"),
                Os::Macos => (
                    "darwin",
                    if host_arch() == "aarch64" {
                        "arm64"
                    } else {
                        "amd64"
                    },
                ),
                Os::Linux => ("linux", "amd64"),
                Os::Freebsd => ("freebsd", "amd64"),
            };
            let name = project
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "app".into());
            let out = project.join(format!(
                "build/crossfire-{}/{}{}",
                t.id(),
                name,
                if t == Os::Windows { ".exe" } else { "" }
            ));
            let mut c = Command::new(go);
            c.args(["build", "-o"])
                .arg(&out)
                .arg(".")
                .current_dir(project)
                .env("GOOS", goos)
                .env("GOARCH", arch);
            if !uses_cgo {
                c.env("CGO_ENABLED", "0");
            }
            run_all(t, vec![c], on, None)
        }
        BuildKind::Node => {
            if t != host {
                return ci(t, "Node/Electron packages are built per OS in CI");
            }
            let Some(npm) = which("npm") else {
                return ci(t, "npm not installed");
            };
            let pkg = std::fs::read_to_string(project.join("package.json")).unwrap_or_default();
            if !pkg.contains("\"build\"") {
                return result(
                    t,
                    BuildStatus::Skipped,
                    String::new(),
                    Some("package.json has no build script".into()),
                );
            }
            let mut install = Command::new(&npm);
            install
                .arg(if project.join("package-lock.json").is_file() {
                    "ci"
                } else {
                    "install"
                })
                .current_dir(project);
            let mut b = Command::new(&npm);
            b.args(["run", "build"]).current_dir(project);
            run_all(t, vec![install, b], on, None)
        }
        BuildKind::Python { entry } => {
            if t != host {
                return ci(t, "PyInstaller bundles must be built on each OS");
            }
            let Some(pyi) = which("pyinstaller") else {
                return result(t, BuildStatus::Skipped, String::new(), Some("PyInstaller not installed (pip install pyinstaller); Python sources run as-is".into()));
            };
            let mut c = Command::new(pyi);
            c.args(["--onefile", "--noconfirm", "--distpath"])
                .arg(project.join(format!("build/crossfire-{}", t.id())))
                .args(["--workpath"])
                .arg(project.join("build/pyinstaller"))
                .arg(entry)
                .current_dir(project);
            run_all(t, vec![c], on, None)
        }
        BuildKind::Engine(e) => result(
            t,
            BuildStatus::Skipped,
            String::new(),
            Some(format!(
                "{e} projects are exported from the editor (export presets) or in CI"
            )),
        ),
        BuildKind::Other => result(
            t,
            BuildStatus::Skipped,
            String::new(),
            Some("no supported build system found".into()),
        ),
    }
}

fn dir_contains(dir: &Path, ext: &str, needle: &str) -> bool {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return false;
    };
    rd.flatten().any(|e| {
        let p = e.path();
        if p.is_dir() {
            let n = e.file_name().to_string_lossy().to_string();
            !n.starts_with('.') && n != "vendor" && n != "build" && dir_contains(&p, ext, needle)
        } else {
            p.extension().and_then(|x| x.to_str()) == Some(ext)
                && std::fs::read_to_string(&p)
                    .map(|s| s.contains(needle))
                    .unwrap_or(false)
        }
    })
}

fn zig_triple(t: Os) -> Option<&'static str> {
    match t {
        Os::Windows => Some("x86_64-windows-gnu"),
        Os::Linux => Some("x86_64-linux-gnu"),
        Os::Freebsd => Some("x86_64-freebsd"),
        Os::Macos => None,
    }
}

fn cmake_system_name(t: Os) -> &'static str {
    match t {
        Os::Windows => "Windows",
        Os::Macos => "Darwin",
        Os::Linux => "Linux",
        Os::Freebsd => "FreeBSD",
    }
}

#[cfg(unix)]
fn make_exec(p: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755));
}
#[cfg(not(unix))]
fn make_exec(_: &Path) {}

/// Write a CMake toolchain file that drives `zig cc -target <triple>`.
fn zig_toolchain(bdir: &Path, zig: &Path, t: Os, triple: &str) -> std::io::Result<PathBuf> {
    let tc = bdir.join("crossfire-toolchain");
    std::fs::create_dir_all(&tc)?;
    let wrap = |name: &str, sub: &str| -> std::io::Result<PathBuf> {
        if cfg!(windows) {
            let p = tc.join(format!("{name}.cmd"));
            std::fs::write(
                &p,
                format!("@echo off\r\n\"{}\" {sub} %*\r\n", zig.display()),
            )?;
            Ok(p)
        } else {
            let p = tc.join(name);
            std::fs::write(
                &p,
                format!("#!/bin/sh\nexec \"{}\" {sub} \"$@\"\n", zig.display()),
            )?;
            make_exec(&p);
            Ok(p)
        }
    };
    let cc = wrap("zcc", &format!("cc -target {triple}"))?;
    let cxx = wrap("zcxx", &format!("c++ -target {triple}"))?;
    let ar = wrap("zar", "ar")?;
    let ranlib = wrap("zranlib", "ranlib")?;
    let rc = wrap("zrc", "rc")?;
    let file = tc.join("toolchain.cmake");
    let mut s = format!(
        "# Generated by Crossfire: cross-compile with zig cc ({triple}).\nset(CMAKE_SYSTEM_NAME {})\nset(CMAKE_SYSTEM_PROCESSOR x86_64)\nset(CMAKE_C_COMPILER \"{}\")\nset(CMAKE_CXX_COMPILER \"{}\")\nset(CMAKE_AR \"{}\" CACHE FILEPATH \"\")\nset(CMAKE_RANLIB \"{}\" CACHE FILEPATH \"\")\n",
        cmake_system_name(t),
        cc.display(),
        cxx.display(),
        ar.display(),
        ranlib.display()
    );
    if t == Os::Windows {
        s.push_str(&format!("set(CMAKE_RC_COMPILER \"{}\")\n", rc.display()));
    }
    s.push_str("set(CMAKE_FIND_ROOT_PATH_MODE_PROGRAM NEVER)\nset(CMAKE_FIND_ROOT_PATH_MODE_LIBRARY ONLY)\nset(CMAKE_FIND_ROOT_PATH_MODE_INCLUDE ONLY)\nset(CMAKE_FIND_ROOT_PATH_MODE_PACKAGE ONLY)\n");
    std::fs::write(&file, s)?;
    Ok(file)
}

fn mingw_toolchain(bdir: &Path) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(bdir)?;
    let file = bdir.join("crossfire-mingw.cmake");
    std::fs::write(
        &file,
        "# Generated by Crossfire: MinGW-w64 cross toolchain.\nset(CMAKE_SYSTEM_NAME Windows)\nset(CMAKE_SYSTEM_PROCESSOR x86_64)\nset(CMAKE_C_COMPILER x86_64-w64-mingw32-gcc)\nset(CMAKE_CXX_COMPILER x86_64-w64-mingw32-g++)\nset(CMAKE_RC_COMPILER x86_64-w64-mingw32-windres)\nset(CMAKE_FIND_ROOT_PATH_MODE_PROGRAM NEVER)\nset(CMAKE_FIND_ROOT_PATH_MODE_LIBRARY ONLY)\nset(CMAKE_FIND_ROOT_PATH_MODE_INCLUDE ONLY)\n",
    )?;
    Ok(file)
}

/// apt packages listed in the generated workflow (for container builds).
fn workflow_apt(project: &Path) -> String {
    let wf = std::fs::read_to_string(project.join(WORKFLOW)).unwrap_or_default();
    wf.lines()
        .find_map(|l| {
            l.split_once("apt-get install -y ")
                .map(|(_, r)| r.trim().to_string())
        })
        .unwrap_or_default()
}

fn cmake_target(project: &Path, t: Os, cfg: &str, on: &dyn Fn(ProgressEvent)) -> BuildTargetResult {
    let host = Os::host();
    let Some(cmake) = which("cmake") else {
        return ci(t, "cmake not installed");
    };
    let bdir = project.join(format!("build/crossfire-{}", t.id()));
    let ninja = which("ninja");
    let configure = |extra: Vec<String>| {
        let mut c = Command::new(&cmake);
        c.arg("-S")
            .arg(project)
            .arg("-B")
            .arg(&bdir)
            .arg(format!("-DCMAKE_BUILD_TYPE={cfg}"));
        if ninja.is_some() && !(host == Os::Windows && t == Os::Windows) {
            c.args(["-G", "Ninja"]);
        }
        c.args(extra);
        c
    };
    let build = || {
        let mut c = Command::new(&cmake);
        c.arg("--build")
            .arg(&bdir)
            .args(["--config", cfg, "--parallel"]);
        c
    };
    if t == host {
        return run_all(
            t,
            vec![configure(vec![]), build()],
            on,
            Some("native host build".into()),
        );
    }
    if t == Os::Macos {
        return ci(
            t,
            "macOS binaries can only be built on a macOS host (Xcode SDK + codesign)",
        );
    }
    // Cross-compile: zig first (all three non-Apple targets), then mingw for Windows.
    if let (Some(zig), Some(triple)) = (which("zig"), zig_triple(t)) {
        match zig_toolchain(&bdir, &zig, t, triple) {
            Ok(tc) => {
                let mut r = run_all(
                    t,
                    vec![configure(vec![format!("-DCMAKE_TOOLCHAIN_FILE={}", tc.display()), "-DCF_USE_SDL3=OFF".into()]), build()],
                    on,
                    Some(format!("cross-compiled with zig cc -target {triple} (SDL3 disabled for the cross build)")),
                );
                if r.status == BuildStatus::Failed {
                    r.reason = Some(format!("zig cross build failed (target system libraries such as X11/ALSA are not in zig's sysroot); see log. {}", r.reason.unwrap_or_default()));
                }
                return r;
            }
            Err(e) => on(ProgressEvent::new(
                Stage::Build,
                Level::Warn,
                -1.0,
                format!("zig toolchain setup failed: {e}"),
            )),
        }
    }
    if t == Os::Windows && which("x86_64-w64-mingw32-gcc").is_some() {
        if let Ok(tc) = mingw_toolchain(&bdir) {
            return run_all(
                t,
                vec![
                    configure(vec![
                        format!("-DCMAKE_TOOLCHAIN_FILE={}", tc.display()),
                        "-DCF_USE_SDL3=OFF".into(),
                    ]),
                    build(),
                ],
                on,
                Some("cross-compiled with MinGW-w64".into()),
            );
        }
    }
    if t == Os::Linux {
        if let Some(docker) = which("docker").or_else(|| which("podman")) {
            let apt = workflow_apt(project);
            let script = format!(
                "apt-get update -qq && apt-get install -y -qq build-essential cmake ninja-build pkg-config git {apt} && cmake -S . -B build/crossfire-linux -G Ninja -DCMAKE_BUILD_TYPE={cfg} && cmake --build build/crossfire-linux --parallel"
            );
            let mut c = Command::new(docker);
            c.args(["run", "--rm", "-v"])
                .arg(format!("{}:/src", project.display()))
                .args(["-w", "/src", "ubuntu:24.04", "bash", "-lc", &script]);
            return run_all(
                t,
                vec![c],
                on,
                Some("built in an ubuntu:24.04 container".into()),
            );
        }
    }
    ci(
        t,
        format!(
            "no cross toolchain for {} on this {} host (install zig{})",
            t.label(),
            host.label(),
            if t == Os::Windows {
                " or mingw-w64"
            } else if t == Os::Linux {
                " or docker"
            } else {
                ""
            }
        ),
    )
}

fn rust_triple(t: Os) -> String {
    match t {
        Os::Windows => if Os::host() == Os::Windows {
            "x86_64-pc-windows-msvc"
        } else {
            "x86_64-pc-windows-gnu"
        }
        .into(),
        Os::Macos => format!("{}-apple-darwin", host_arch()),
        Os::Linux => format!(
            "{}-unknown-linux-gnu",
            if Os::host() == Os::Linux {
                host_arch()
            } else {
                "x86_64"
            }
        ),
        Os::Freebsd => "x86_64-unknown-freebsd".into(),
    }
}

fn cargo_target(
    project: &Path,
    t: Os,
    release: bool,
    on: &dyn Fn(ProgressEvent),
) -> BuildTargetResult {
    let Some(cargo) = which("cargo") else {
        return ci(t, "Rust toolchain not installed");
    };
    let mut c = Command::new(&cargo);
    c.current_dir(project);
    if t == Os::host() {
        c.arg("build");
        if release {
            c.arg("--release");
        }
        return run_all(t, vec![c], on, Some("native host build".into()));
    }
    if t == Os::Macos {
        return ci(t, "macOS binaries need a macOS host (Apple SDK)");
    }
    let triple = rust_triple(t);
    let installed = rustup_targets().contains(&triple);
    if which("cargo-zigbuild").is_some() && which("zig").is_some() && t != Os::Freebsd {
        c.args(["zigbuild", "--target", &triple]);
    } else if installed {
        c.args(["build", "--target", &triple]);
    } else {
        return ci(t, format!("rustup target {triple} not installed (rustup target add {triple}) and cargo-zigbuild unavailable"));
    }
    if release {
        c.arg("--release");
    }
    let mut r = run_all(t, vec![c], on, Some(format!("cross-compiled for {triple}")));
    if r.status == BuildStatus::Success {
        let dir =
            project
                .join("target")
                .join(&triple)
                .join(if release { "release" } else { "debug" });
        r.artifacts = find_artifacts(&dir, SystemTime::UNIX_EPOCH);
    }
    r
}

// ---------------------------------------------------------------------------
// Artifacts & packaging
// ---------------------------------------------------------------------------

const SKIP_ARTIFACT_DIRS: &[&str] = &[
    "CMakeFiles",
    "_deps",
    ".git",
    "node_modules",
    "obj",
    "deps",
    ".fingerprint",
    "incremental",
    "crossfire-toolchain",
    "pyinstaller",
    "AppDir",
    "stage",
    "src",
    "include",
];

fn is_binary(p: &Path) -> bool {
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if matches!(
        ext.as_str(),
        "o" | "obj" | "class" | "a" | "lib" | "pdb" | "rlib" | "d" | "rmeta"
    ) {
        return false;
    }
    let Ok(mut f) = std::fs::File::open(p) else {
        return false;
    };
    let mut buf = [0u8; 4];
    use std::io::Read;
    if f.read_exact(&mut buf).is_err() {
        return false;
    }
    buf[..2] == *b"MZ"
        || buf == *b"\x7fELF"
        || matches!(
            u32::from_be_bytes(buf),
            0xfeedface | 0xfeedfacf | 0xcefaedfe | 0xcffaedfe | 0xcafebabe
        )
}

/// Binaries / bundles under `dir` modified at or after `since`.
pub fn find_artifacts(dir: &Path, since: SystemTime) -> Vec<String> {
    let mut out = Vec::new();
    fn walk(d: &Path, since: SystemTime, depth: usize, out: &mut Vec<String>) {
        if depth > 6 || out.len() >= 100 {
            return;
        }
        let Ok(rd) = std::fs::read_dir(d) else { return };
        for e in rd.flatten() {
            let p = e.path();
            let name = e.file_name().to_string_lossy().to_string();
            let Ok(meta) = e.metadata() else { continue };
            let fresh = meta.modified().map(|m| m >= since).unwrap_or(true);
            if meta.is_dir() {
                if name.ends_with(".app") {
                    if fresh {
                        out.push(p.to_string_lossy().to_string());
                    }
                    continue;
                }
                if SKIP_ARTIFACT_DIRS.contains(&name.as_str()) || name.starts_with('.') {
                    continue;
                }
                walk(&p, since, depth + 1, out);
            } else if fresh
                && (is_binary(&p)
                    || name.ends_with(".dmg")
                    || name.ends_with(".AppImage")
                    || name.ends_with(".pkg"))
            {
                out.push(p.to_string_lossy().to_string());
            }
        }
    }
    walk(dir, since, 0, &mut out);
    out.sort();
    out
}

fn archive(files: &[String], out: &Path, zip: bool, on: &dyn Fn(ProgressEvent)) -> bool {
    let Some(cmake) = which("cmake") else {
        return false;
    };
    let Some(parent) = files
        .first()
        .and_then(|f| Path::new(f).parent().map(Path::to_path_buf))
    else {
        return false;
    };
    let mut c = Command::new(cmake);
    c.current_dir(&parent)
        .args(["-E", "tar", if zip { "cf" } else { "czf" }])
        .arg(out);
    if zip {
        c.arg("--format=zip");
    }
    for f in files {
        c.arg(Path::new(f).strip_prefix(&parent).unwrap_or(Path::new(f)));
    }
    run_streaming(c, Stage::Build, on, None).success
}

/// Product name: CMake/Cargo/package.json project name, else the directory name.
fn project_name(project: &Path) -> String {
    let read = |f: &str| std::fs::read_to_string(project.join(f)).unwrap_or_default();
    let cmake = regex::Regex::new(r"(?i)project\s*\(\s*([A-Za-z0-9_\-]+)").unwrap();
    if let Some(c) = cmake.captures(&read("CMakeLists.txt")) {
        return c[1].to_string();
    }
    if let Ok(v) = read("Cargo.toml").parse::<toml::Table>() {
        if let Some(n) = v
            .get("package")
            .and_then(|p| p.get("name"))
            .and_then(|n| n.as_str())
        {
            return n.to_string();
        }
    }
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&read("package.json")) {
        if let Some(n) = v.get("name").and_then(|n| n.as_str()) {
            return n.to_string();
        }
    }
    project
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "app".into())
}

fn package(project: &Path, t: Os, r: &mut BuildTargetResult, on: &dyn Fn(ProgressEvent)) {
    let name = project_name(project);
    let bdir = project.join(format!("build/crossfire-{}", t.id()));
    let _ = std::fs::create_dir_all(&bdir);
    // Only archive artifacts living next to each other (same directory as the first).
    let first_dir = r
        .artifacts
        .first()
        .and_then(|a| Path::new(a).parent().map(Path::to_path_buf));
    let group: Vec<String> = r
        .artifacts
        .iter()
        .filter(|a| Path::new(a).parent().map(Path::to_path_buf) == first_dir)
        .cloned()
        .collect();
    let mut produced: Vec<String> = Vec::new();
    match t {
        Os::Macos => {
            if let (Some(app), Some(hdiutil)) = (
                r.artifacts.iter().find(|a| a.ends_with(".app")).cloned(),
                which("hdiutil"),
            ) {
                let dmg = bdir.join(format!("{name}.dmg"));
                let mut c = Command::new(hdiutil);
                c.args([
                    "create",
                    "-volname",
                    &name,
                    "-srcfolder",
                    &app,
                    "-ov",
                    "-format",
                    "UDZO",
                ])
                .arg(&dmg);
                if run_streaming(c, Stage::Build, on, None).success {
                    produced.push(dmg.to_string_lossy().to_string());
                }
            } else if !group.is_empty() {
                let zip = bdir.join(format!("{name}-macos.zip"));
                if archive(&group, &zip, true, on) {
                    produced.push(zip.to_string_lossy().to_string());
                }
            }
        }
        Os::Linux
            if Os::host() == Os::Linux
                && project.join("packaging/linux/build-appimage.sh").is_file()
                && which("linuxdeploy").is_some() =>
        {
            let mut c = Command::new("bash");
            c.arg(project.join("packaging/linux/build-appimage.sh"))
                .arg(&bdir)
                .current_dir(project);
            if run_streaming(c, Stage::Build, on, None).success {
                produced.extend(
                    find_artifacts(&bdir, SystemTime::UNIX_EPOCH)
                        .into_iter()
                        .filter(|a| a.ends_with(".AppImage")),
                );
            }
        }
        Os::Freebsd
            if Os::host() == Os::Freebsd
                && project.join("packaging/freebsd/make-pkg.sh").is_file() =>
        {
            let mut c = Command::new("sh");
            c.arg(project.join("packaging/freebsd/make-pkg.sh"))
                .arg(&bdir)
                .current_dir(project);
            if run_streaming(c, Stage::Build, on, None).success {
                produced.extend(
                    find_artifacts(&bdir, SystemTime::UNIX_EPOCH)
                        .into_iter()
                        .filter(|a| a.ends_with(".pkg")),
                );
            }
        }
        _ => {
            if !group.is_empty() {
                let (file, zip) = if t == Os::Windows {
                    (format!("{name}-windows-x64.zip"), true)
                } else {
                    (format!("{name}-{}.tar.gz", t.id()), false)
                };
                let outp = bdir.join(file);
                if archive(&group, &outp, zip, on) {
                    produced.push(outp.to_string_lossy().to_string());
                }
            }
        }
    }
    if produced.is_empty() {
        on(ProgressEvent::new(
            Stage::Build,
            Level::Warn,
            -1.0,
            format!("{}: nothing packaged", t.label()),
        ));
    }
    r.artifacts.extend(produced);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_kinds() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("Cargo.toml"), "[package]\nname='x'\n").unwrap();
        assert_eq!(detect_kind(d.path()), BuildKind::Cargo);
        std::fs::write(d.path().join("CMakeLists.txt"), "").unwrap();
        assert_eq!(detect_kind(d.path()), BuildKind::CMake);
    }

    #[test]
    fn macos_from_other_host_is_ci() {
        if Os::host() == Os::Macos {
            return;
        }
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("CMakeLists.txt"), "").unwrap();
        let r = build(
            &BuildRequest {
                project: d.path().to_string_lossy().into(),
                targets: vec![Os::Macos],
                release: true,
                package: false,
            },
            &|_| {},
        )
        .unwrap();
        assert_eq!(r.results[0].status, BuildStatus::Ci);
    }
}
