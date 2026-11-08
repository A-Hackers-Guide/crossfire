//! Detect compilers, build tools, cross-compilers and runtimes on the host.

use crate::model::{Os, Toolchain};
use crate::util::{capture, version_line, which};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// How a tool's producible targets are derived.
#[derive(Clone, Copy)]
enum Targets {
    None,
    Host,
    Fixed(&'static [Os]),
    /// Fixed set, plus the host OS.
    FixedPlusHost(&'static [Os]),
}

struct Spec {
    id: &'static str,
    name: &'static str,
    exes: &'static [&'static str],
    version_args: &'static [&'static str],
    targets: Targets,
    /// Install hints: (brew, apt, winget, pkg)
    hint: (&'static str, &'static str, &'static str, &'static str),
}

const ALL: &[Os] = &[Os::Windows, Os::Macos, Os::Linux, Os::Freebsd];

const SPECS: &[Spec] = &[
    Spec {
        id: "cc",
        name: "C compiler (cc)",
        exes: &["cc"],
        version_args: &["--version"],
        targets: Targets::Host,
        hint: (
            "xcode-select --install",
            "sudo apt install build-essential",
            "winget install Microsoft.VisualStudio.2022.BuildTools",
            "(base system)",
        ),
    },
    Spec {
        id: "clang",
        name: "Clang",
        exes: &["clang"],
        version_args: &["--version"],
        targets: Targets::Host,
        hint: (
            "xcode-select --install",
            "sudo apt install clang",
            "winget install LLVM.LLVM",
            "(base system)",
        ),
    },
    Spec {
        id: "gcc",
        name: "GCC",
        exes: &["gcc", "gcc14", "gcc13"],
        version_args: &["--version"],
        targets: Targets::Host,
        hint: (
            "brew install gcc",
            "sudo apt install gcc g++",
            "winget install MSYS2.MSYS2",
            "pkg install gcc",
        ),
    },
    Spec {
        id: "cmake",
        name: "CMake",
        exes: &["cmake"],
        version_args: &["--version"],
        targets: Targets::Host,
        hint: (
            "brew install cmake",
            "sudo apt install cmake",
            "winget install Kitware.CMake",
            "pkg install cmake",
        ),
    },
    Spec {
        id: "ninja",
        name: "Ninja",
        exes: &["ninja"],
        version_args: &["--version"],
        targets: Targets::Host,
        hint: (
            "brew install ninja",
            "sudo apt install ninja-build",
            "winget install Ninja-build.Ninja",
            "pkg install ninja",
        ),
    },
    Spec {
        id: "make",
        name: "Make",
        exes: &["gmake", "make"],
        version_args: &["--version"],
        targets: Targets::Host,
        hint: (
            "xcode-select --install",
            "sudo apt install make",
            "winget install GnuWin32.Make",
            "pkg install gmake",
        ),
    },
    Spec {
        id: "meson",
        name: "Meson",
        exes: &["meson"],
        version_args: &["--version"],
        targets: Targets::Host,
        hint: (
            "brew install meson",
            "sudo apt install meson",
            "pip install meson",
            "pkg install meson",
        ),
    },
    Spec {
        id: "zig",
        name: "Zig (zig cc cross-compiler)",
        exes: &["zig"],
        version_args: &["version"],
        targets: Targets::FixedPlusHost(&[Os::Windows, Os::Linux, Os::Freebsd]),
        hint: (
            "brew install zig",
            "sudo snap install zig --classic",
            "winget install zig.zig",
            "pkg install zig",
        ),
    },
    Spec {
        id: "mingw",
        name: "MinGW-w64 (x86_64-w64-mingw32-gcc)",
        exes: &["x86_64-w64-mingw32-gcc"],
        version_args: &["--version"],
        targets: Targets::Fixed(&[Os::Windows]),
        hint: (
            "brew install mingw-w64",
            "sudo apt install mingw-w64",
            "winget install MSYS2.MSYS2",
            "pkg install mingw-w64",
        ),
    },
    Spec {
        id: "xcodebuild",
        name: "Xcode (xcodebuild)",
        exes: &["xcodebuild"],
        version_args: &["-version"],
        targets: Targets::Fixed(&[Os::Macos]),
        hint: (
            "Install Xcode from the App Store",
            "macOS only",
            "macOS only",
            "macOS only",
        ),
    },
    Spec {
        id: "xcrun",
        name: "Xcode command line tools (xcrun)",
        exes: &["xcrun"],
        version_args: &["--version"],
        targets: Targets::Fixed(&[Os::Macos]),
        hint: (
            "xcode-select --install",
            "macOS only",
            "macOS only",
            "macOS only",
        ),
    },
    Spec {
        id: "cargo",
        name: "Rust (cargo)",
        exes: &["cargo"],
        version_args: &["--version"],
        targets: Targets::Host,
        hint: (
            "brew install rustup && rustup-init",
            "curl https://sh.rustup.rs -sSf | sh",
            "winget install Rustlang.Rustup",
            "pkg install rust",
        ),
    },
    Spec {
        id: "cargo-zigbuild",
        name: "cargo-zigbuild",
        exes: &["cargo-zigbuild"],
        version_args: &["--version"],
        targets: Targets::FixedPlusHost(&[Os::Windows, Os::Linux]),
        hint: (
            "cargo install cargo-zigbuild",
            "cargo install cargo-zigbuild",
            "cargo install cargo-zigbuild",
            "cargo install cargo-zigbuild",
        ),
    },
    Spec {
        id: "dotnet",
        name: ".NET SDK",
        exes: &["dotnet"],
        version_args: &["--version"],
        targets: Targets::Fixed(&[Os::Windows, Os::Macos, Os::Linux]),
        hint: (
            "brew install --cask dotnet-sdk",
            "sudo apt install dotnet-sdk-8.0",
            "winget install Microsoft.DotNet.SDK.8",
            "pkg install dotnet",
        ),
    },
    Spec {
        id: "go",
        name: "Go",
        exes: &["go"],
        version_args: &["version"],
        targets: Targets::Fixed(ALL),
        hint: (
            "brew install go",
            "sudo apt install golang",
            "winget install GoLang.Go",
            "pkg install go",
        ),
    },
    Spec {
        id: "java",
        name: "Java (JDK)",
        exes: &["java"],
        version_args: &["-version"],
        targets: Targets::Fixed(ALL),
        hint: (
            "brew install openjdk",
            "sudo apt install openjdk-21-jdk",
            "winget install Microsoft.OpenJDK.21",
            "pkg install openjdk21",
        ),
    },
    Spec {
        id: "gradle",
        name: "Gradle",
        exes: &["gradle"],
        version_args: &["--version"],
        targets: Targets::Fixed(ALL),
        hint: (
            "brew install gradle",
            "sudo apt install gradle",
            "winget install Gradle.Gradle",
            "pkg install gradle",
        ),
    },
    Spec {
        id: "maven",
        name: "Maven",
        exes: &["mvn"],
        version_args: &["--version"],
        targets: Targets::Fixed(ALL),
        hint: (
            "brew install maven",
            "sudo apt install maven",
            "winget install Apache.Maven",
            "pkg install maven",
        ),
    },
    Spec {
        id: "python",
        name: "Python 3",
        exes: &["python3", "python"],
        version_args: &["--version"],
        targets: Targets::Host,
        hint: (
            "brew install python",
            "sudo apt install python3",
            "winget install Python.Python.3.12",
            "pkg install python3",
        ),
    },
    Spec {
        id: "pyinstaller",
        name: "PyInstaller",
        exes: &["pyinstaller"],
        version_args: &["--version"],
        targets: Targets::Host,
        hint: (
            "pip install pyinstaller",
            "pip install pyinstaller",
            "pip install pyinstaller",
            "pip install pyinstaller",
        ),
    },
    Spec {
        id: "node",
        name: "Node.js",
        exes: &["node"],
        version_args: &["--version"],
        targets: Targets::Host,
        hint: (
            "brew install node",
            "sudo apt install nodejs npm",
            "winget install OpenJS.NodeJS.LTS",
            "pkg install node",
        ),
    },
    Spec {
        id: "docker",
        name: "Docker",
        exes: &["docker"],
        version_args: &["--version"],
        targets: Targets::Fixed(&[Os::Linux]),
        hint: (
            "brew install --cask docker",
            "sudo apt install docker.io",
            "winget install Docker.DockerDesktop",
            "Docker is not available on FreeBSD (use jails/bhyve)",
        ),
    },
    Spec {
        id: "podman",
        name: "Podman",
        exes: &["podman"],
        version_args: &["--version"],
        targets: Targets::Fixed(&[Os::Linux]),
        hint: (
            "brew install podman",
            "sudo apt install podman",
            "winget install RedHat.Podman",
            "pkg install podman",
        ),
    },
    Spec {
        id: "godot",
        name: "Godot",
        exes: &["godot", "godot4", "Godot"],
        version_args: &["--version"],
        targets: Targets::Fixed(ALL),
        hint: (
            "brew install --cask godot",
            "flatpak install flathub org.godotengine.Godot",
            "winget install GodotEngine.GodotEngine",
            "pkg install godot",
        ),
    },
    Spec {
        id: "wine",
        name: "Wine",
        exes: &["wine", "wine64"],
        version_args: &["--version"],
        targets: Targets::None,
        hint: (
            "brew install --cask wine-stable",
            "sudo apt install wine",
            "Windows runs .exe natively",
            "pkg install wine",
        ),
    },
    Spec {
        id: "linuxdeploy",
        name: "linuxdeploy (AppImage)",
        exes: &["linuxdeploy", "linuxdeploy-x86_64.AppImage"],
        version_args: &["--version"],
        targets: Targets::None,
        hint: (
            "Linux only (runs in CI)",
            "Download from github.com/linuxdeploy/linuxdeploy/releases",
            "Linux only",
            "Linux only",
        ),
    },
    Spec {
        id: "appimagetool",
        name: "appimagetool",
        exes: &["appimagetool", "appimagetool-x86_64.AppImage"],
        version_args: &["--version"],
        targets: Targets::None,
        hint: (
            "Linux only (runs in CI)",
            "Download from github.com/AppImage/appimagetool/releases",
            "Linux only",
            "Linux only",
        ),
    },
    Spec {
        id: "makensis",
        name: "NSIS (makensis)",
        exes: &["makensis"],
        version_args: &["-VERSION"],
        targets: Targets::None,
        hint: (
            "brew install makensis",
            "sudo apt install nsis",
            "winget install NSIS.NSIS",
            "pkg install nsis",
        ),
    },
    Spec {
        id: "vcpkg",
        name: "vcpkg",
        exes: &["vcpkg"],
        version_args: &["version"],
        targets: Targets::None,
        hint: (
            "brew install vcpkg",
            "git clone https://github.com/microsoft/vcpkg && ./vcpkg/bootstrap-vcpkg.sh",
            "winget install Microsoft.vcpkg",
            "pkg install vcpkg",
        ),
    },
];

fn hint_for_host(h: &(&str, &str, &str, &str)) -> String {
    match Os::host() {
        Os::Macos => h.0.to_string(),
        Os::Linux => h.1.to_string(),
        Os::Windows => h.2.to_string(),
        Os::Freebsd => h.3.to_string(),
    }
}

fn resolve_targets(t: Targets, found: bool) -> Vec<Os> {
    if !found {
        return vec![];
    }
    let host = Os::host();
    let mut v = match t {
        Targets::None => vec![],
        Targets::Host => vec![host],
        Targets::Fixed(f) => f.to_vec(),
        Targets::FixedPlusHost(f) => {
            let mut v = f.to_vec();
            v.push(host);
            v
        }
    };
    v.sort();
    v.dedup();
    v
}

/// Tools whose binaries are never executed for a version probe (they are or
/// may be GUI launchers: running them opens windows / Dock icons).
const NO_EXEC: &[&str] = &["wine", "godot"];

/// Command-line tools that are stubs on macOS until the Xcode Command Line
/// Tools are installed (running a stub pops up an installer dialog).
const MAC_CLT_STUBS: &[&str] = &[
    "cc",
    "clang",
    "gcc",
    "make",
    "python",
    "xcrun",
    "xcodebuild",
];

/// Version of a tool without side effects: bundle binaries (including
/// symlinks into an .app) are read from Info.plist, GUI-ish tools are not run,
/// and macOS stub binaries are only run when their backing install exists.
fn safe_version(spec: &Spec, p: &Path) -> Option<String> {
    let real = p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    if real.to_string_lossy().contains(".app/") {
        return bundle_version(&real);
    }
    if NO_EXEC.contains(&spec.id) {
        return None;
    }
    if cfg!(target_os = "macos") && real.starts_with("/usr/bin") {
        let clt = Path::new("/Library/Developer/CommandLineTools/usr/bin").is_dir()
            || Path::new("/Applications/Xcode.app").is_dir();
        if MAC_CLT_STUBS.contains(&spec.id) && !clt {
            return None;
        }
        if spec.id == "java" {
            let has_jdk = std::env::var_os("JAVA_HOME").is_some()
                || std::fs::read_dir("/Library/Java/JavaVirtualMachines")
                    .map(|mut d| d.next().is_some())
                    .unwrap_or(false);
            if !has_jdk {
                return None;
            }
        }
    }
    version_line(p, spec.version_args)
}

fn probe(spec: &Spec) -> Toolchain {
    let path = spec.exes.iter().find_map(|e| which(e));
    let version = path.as_ref().and_then(|p| safe_version(spec, p));
    let found = path.is_some();
    Toolchain {
        id: spec.id.into(),
        name: spec.name.into(),
        found,
        version,
        path: path.map(|p| p.to_string_lossy().to_string()),
        targets: resolve_targets(spec.targets, found),
        hint: hint_for_host(&spec.hint),
    }
}

/// Map an installed rustup target triple to an OS.
fn triple_os(t: &str) -> Option<Os> {
    if t.contains("windows") {
        Some(Os::Windows)
    } else if t.contains("apple-darwin") {
        Some(Os::Macos)
    } else if t.contains("linux") {
        Some(Os::Linux)
    } else if t.contains("freebsd") {
        Some(Os::Freebsd)
    } else {
        None
    }
}

/// Installed rustup targets (triples).
pub fn rustup_targets() -> Vec<String> {
    let Some(rustup) = which("rustup") else {
        return vec![];
    };
    capture(
        &rustup,
        &["target", "list", "--installed"],
        Duration::from_secs(15),
    )
    .map(|(_, out)| {
        out.lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.contains(' '))
            .map(String::from)
            .collect()
    })
    .unwrap_or_default()
}

/// Game Porting Toolkit's wine binary, when installed.
pub fn gptk_wine() -> Option<PathBuf> {
    let mut cands = vec![
        PathBuf::from("/Applications/Game Porting Toolkit.app/Contents/MacOS/wine"),
        PathBuf::from("/Applications/Game Porting Toolkit.app/Contents/Resources/wine/bin/wine64"),
        PathBuf::from("/usr/local/opt/game-porting-toolkit/bin/wine64"),
        PathBuf::from("/opt/homebrew/opt/game-porting-toolkit/bin/wine64"),
    ];
    if let Some(h) = std::env::var_os("HOME") {
        cands.push(Path::new(&h).join("Applications/Game Porting Toolkit.app/Contents/MacOS/wine"));
    }
    cands.into_iter().find(|p| p.is_file())
}

fn msvc() -> Toolchain {
    let mut tc = Toolchain {
        id: "msvc".into(),
        name: "MSVC (cl.exe)".into(),
        found: false,
        version: None,
        path: None,
        targets: vec![],
        hint: if Os::host() == Os::Windows {
            "winget install Microsoft.VisualStudio.2022.BuildTools --override \"--add Microsoft.VisualStudio.Workload.VCTools\"".into()
        } else {
            "Windows only — build Windows targets with zig/mingw locally or in CI (windows-latest)"
                .into()
        },
    };
    if Os::host() != Os::Windows {
        return tc;
    }
    if let Some(cl) = which("cl") {
        tc.found = true;
        tc.path = Some(cl.to_string_lossy().to_string());
    } else {
        let pf =
            std::env::var("ProgramFiles(x86)").unwrap_or_else(|_| "C:\\Program Files (x86)".into());
        let vswhere = PathBuf::from(pf).join("Microsoft Visual Studio\\Installer\\vswhere.exe");
        if vswhere.is_file() {
            if let Some((_, out)) = capture(
                &vswhere,
                &[
                    "-latest",
                    "-products",
                    "*",
                    "-requires",
                    "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
                    "-property",
                    "installationPath",
                ],
                Duration::from_secs(15),
            ) {
                let inst = out.lines().next().unwrap_or("").trim().to_string();
                if !inst.is_empty() {
                    tc.found = true;
                    tc.path = Some(inst.clone());
                    tc.version = capture(
                        &vswhere,
                        &["-latest", "-property", "catalog_productDisplayVersion"],
                        Duration::from_secs(15),
                    )
                    .map(|(_, v)| v.trim().to_string());
                }
            }
        }
    }
    if tc.found {
        tc.targets = vec![Os::Windows];
    }
    tc
}

/// Detect every known toolchain.
pub fn toolchains() -> Vec<Toolchain> {
    // Version probes are independent; run them in parallel.
    let mut out: Vec<Toolchain> = std::thread::scope(|s| {
        let hs: Vec<_> = SPECS.iter().map(|sp| s.spawn(move || probe(sp))).collect();
        hs.into_iter().filter_map(|h| h.join().ok()).collect()
    });
    // Rust: add installed rustup targets.
    if let Some(cargo) = out.iter_mut().find(|t| t.id == "cargo" && t.found) {
        for t in rustup_targets() {
            if let Some(o) = triple_os(&t) {
                if !cargo.targets.contains(&o) {
                    cargo.targets.push(o);
                }
            }
        }
        cargo.targets.sort();
    }
    out.push(msvc());
    // Game Porting Toolkit (macOS only).
    let gptk = gptk_wine();
    out.push(Toolchain {
        id: "gptk".into(),
        name: "Game Porting Toolkit (Wine + D3DMetal)".into(),
        found: gptk.is_some(),
        // Never execute the GPTK binary just to probe it: it lives in an .app bundle,
        // so every launch spawns a Dock icon. Read the bundle's Info.plist instead.
        version: gptk.as_ref().and_then(|p| bundle_version(p)),
        path: gptk.map(|p| p.to_string_lossy().to_string()),
        targets: vec![],
        hint: if Os::host() == Os::Macos {
            "Download Game Porting Toolkit from developer.apple.com/games (runs Windows builds on Apple Silicon)".into()
        } else {
            "macOS only".into()
        },
    });
    for (id, name, app, hint) in [
        (
            "unity",
            "Unity Hub",
            unity_hub(),
            "Install Unity Hub from unity.com/download (exports Windows/macOS/Linux)",
        ),
        (
            "crossover",
            "CrossOver",
            mac_app("CrossOver.app"),
            "codeweavers.com/crossover (commercial Wine for macOS/Linux)",
        ),
    ] {
        let targets = if id == "unity" && app.is_some() {
            vec![Os::Windows, Os::Macos, Os::Linux]
        } else {
            vec![]
        };
        out.push(Toolchain {
            id: id.into(),
            name: name.into(),
            found: app.is_some(),
            version: None,
            path: app.map(|p| p.to_string_lossy().to_string()),
            targets,
            hint: hint.into(),
        });
    }
    out
}

fn mac_app(name: &str) -> Option<PathBuf> {
    let p = PathBuf::from("/Applications").join(name);
    p.exists().then_some(p)
}

fn unity_hub() -> Option<PathBuf> {
    [
        PathBuf::from("/Applications/Unity Hub.app"),
        PathBuf::from("C:\\Program Files\\Unity Hub\\Unity Hub.exe"),
        PathBuf::from("/opt/unityhub/unityhub"),
    ]
    .into_iter()
    .find(|p| p.exists())
    .or_else(|| which("unityhub"))
}

/// Read CFBundleShortVersionString from the .app bundle that contains `exe`, without running it.
fn bundle_version(exe: &Path) -> Option<String> {
    let contents = exe
        .ancestors()
        .find(|a| a.file_name().is_some_and(|n| n == "Contents"))?;
    let plist = std::fs::read_to_string(contents.join("Info.plist")).ok()?;
    let key = plist.find("<key>CFBundleShortVersionString</key>")?;
    let rest = &plist[key..];
    let start = rest.find("<string>")? + "<string>".len();
    let end = rest[start..].find("</string>")?;
    Some(rest[start..start + end].trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triples_map_to_os() {
        assert_eq!(triple_os("x86_64-pc-windows-gnu"), Some(Os::Windows));
        assert_eq!(triple_os("aarch64-apple-darwin"), Some(Os::Macos));
        assert_eq!(triple_os("x86_64-unknown-linux-gnu"), Some(Os::Linux));
        assert_eq!(triple_os("x86_64-unknown-freebsd"), Some(Os::Freebsd));
        assert_eq!(triple_os("wasm32-unknown-unknown"), None);
    }

    #[test]
    fn detects_some_toolchains() {
        let t = toolchains();
        assert!(t.iter().any(|x| x.id == "cmake"));
        assert!(t.iter().all(|x| !x.hint.is_empty()));
        // A found compiler can always build for the host.
        for x in t
            .iter()
            .filter(|x| x.found && (x.id == "cc" || x.id == "clang"))
        {
            assert!(x.targets.contains(&Os::host()));
        }
    }
}
