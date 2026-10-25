//! Project-kind and dependency detection from well-known project files.

use crate::model::{Category, Dependency, Finding, Os, ProjectKind};
use crate::scan::{read_text, FileInfo};
use regex::Regex;
use std::collections::BTreeSet;
use std::path::Path;
use std::sync::LazyLock;

/// Compile a regex once per call site.
macro_rules! re {
    ($s:expr) => {{
        static R: LazyLock<Regex> = LazyLock::new(|| Regex::new($s).unwrap());
        &*R
    }};
}

pub struct Detection {
    pub kinds: Vec<ProjectKind>,
    pub dependencies: Vec<Dependency>,
    /// Extra OS evidence from project files (weight added to finding scores).
    pub os_hints: Vec<(Os, f64)>,
    /// Project name from a manifest, when one declares it.
    pub name: Option<String>,
    /// Overall flavor: game | app | library | cli | service | unknown.
    pub flavor: String,
}

fn file_name(rel: &str) -> &str {
    rel.rsplit('/').next().unwrap_or(rel)
}

fn depth(rel: &str) -> usize {
    rel.matches('/').count()
}

fn ext(rel: &str) -> String {
    Path::new(rel).extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default()
}

struct Ctx<'a> {
    root: &'a Path,
    files: &'a [FileInfo],
    findings: &'a [Finding],
}

impl Ctx<'_> {
    fn read(&self, rel: &str) -> String {
        read_text(&self.root.join(rel)).unwrap_or_default()
    }
    /// Shallowest file with this exact (case-insensitive) name.
    fn find_name(&self, name: &str) -> Option<&str> {
        self.files
            .iter()
            .filter(|f| file_name(&f.rel).eq_ignore_ascii_case(name))
            .min_by_key(|f| depth(&f.rel))
            .map(|f| f.rel.as_str())
    }
    fn find_ext(&self, e: &str) -> Vec<&str> {
        let mut v: Vec<&str> = self.files.iter().filter(|f| ext(&f.rel) == e).map(|f| f.rel.as_str()).collect();
        v.sort_by_key(|r| depth(r));
        v
    }
    fn has_api(&self, pred: impl Fn(&Finding) -> bool) -> bool {
        self.findings.iter().any(pred)
    }
}

/// Run all detectors.
pub fn detect(root: &Path, files: &[FileInfo], findings: &[Finding], frameworks: &[String]) -> Detection {
    let cx = Ctx { root, files, findings };
    let mut kinds: Vec<ProjectKind> = Vec::new();
    let mut deps: Vec<Dependency> = Vec::new();
    let mut hints: Vec<(Os, f64)> = Vec::new();
    let mut name: Option<String> = None;
    let mut inherent: Vec<(usize, &'static str)> = Vec::new(); // kind index -> fixed flavor
    let mut add = |kinds: &mut Vec<ProjectKind>, n: &str, evidence: String, fixed: Option<&'static str>| {
        if kinds.iter().any(|k| k.name == n) {
            return;
        }
        if let Some(f) = fixed {
            inherent.push((kinds.len(), f));
        }
        kinds.push(ProjectKind { name: n.to_string(), flavor: String::new(), evidence });
    };

    // --- build systems -----------------------------------------------------
    if let Some(p) = cx.find_name("CMakeLists.txt") {
        let txt = cx.read(p);
        add(&mut kinds, "CMake", p.to_string(), None);
        for c in re!(r"(?i)find_package\s*\(\s*([A-Za-z0-9_]+)").captures_iter(&txt) {
            push_dep(&mut deps, &c[1], p);
        }
        if let Some(c) = re!(r"(?i)project\s*\(\s*([A-Za-z0-9_\-]+)").captures(&txt) {
            name.get_or_insert(c[1].to_string());
        }
        if txt.contains("find_package(Qt") {
            add(&mut kinds, "Qt", format!("{p} (find_package(Qt…))"), Some("app"));
        }
        for lib in system_libs_in(&txt) {
            push_dep(&mut deps, &lib, p);
        }
    }
    for (file, kind, ev) in [
        ("Makefile", "Make", ""),
        ("GNUmakefile", "Make", ""),
        ("meson.build", "Meson", ""),
        ("premake5.lua", "Premake", ""),
        ("premake4.lua", "Premake", ""),
        ("SConstruct", "SCons", ""),
        ("configure.ac", "Autotools", ""),
        ("configure.in", "Autotools", ""),
        ("build.zig", "Zig", ""),
        ("xmake.lua", "xmake", ""),
        ("MODULE.bazel", "Bazel", ""),
        ("WORKSPACE", "Bazel", ""),
        ("Package.swift", "Swift PM", ""),
        ("go.mod", "Go", ""),
        ("pubspec.yaml", "Dart/Flutter", ""),
        ("build.gradle", "Gradle", ""),
        ("build.gradle.kts", "Gradle", ""),
        ("pom.xml", "Maven", ""),
        ("Dockerfile", "Docker", " (container build)"),
    ] {
        if let Some(p) = cx.find_name(file) {
            add(&mut kinds, kind, format!("{p}{ev}"), None);
        }
    }
    if let Some(p) = cx.find_name("Makefile") {
        let t = cx.read(p);
        if t.contains("PORTNAME") && t.contains("bsd.port.mk") {
            add(&mut kinds, "FreeBSD port", p.to_string(), None);
            hints.push((Os::Freebsd, 6.0));
        }
        for c in re!(r"-l([A-Za-z0-9_+\-]+)").captures_iter(&t) {
            push_dep(&mut deps, &c[1], p);
        }
    }
    if let Some(p) = cx.find_name("meson.build") {
        let t = cx.read(p);
        for c in re!(r#"dependency\s*\(\s*['"]([\w\-+.]+)['"]"#).captures_iter(&t) {
            push_dep(&mut deps, &c[1], p);
        }
    }

    // --- MSBuild / Visual Studio ------------------------------------------------
    let slns = cx.find_ext("sln");
    let vcx = cx.find_ext("vcxproj");
    if let Some(p) = slns.first().or(vcx.first()) {
        add(&mut kinds, "MSBuild", format!("{p}{}", if vcx.is_empty() { "" } else { " (Visual C++)" }), None);
        hints.push((Os::Windows, 4.0));
    }
    for p in &vcx {
        let t = cx.read(p);
        for c in re!(r"<AdditionalDependencies>([^<]+)</AdditionalDependencies>").captures_iter(&t) {
            for lib in c[1].split(';') {
                let lib = lib.trim();
                if lib.ends_with(".lib") {
                    push_dep(&mut deps, lib.trim_end_matches(".lib"), p);
                }
            }
        }
    }

    // --- .NET ------------------------------------------------------------------
    let mut projs = cx.find_ext("csproj");
    projs.extend(cx.find_ext("fsproj"));
    projs.extend(cx.find_ext("vbproj"));
    for p in &projs {
        let t = cx.read(p);
        let mut notes = Vec::new();
        let tfm = re!(r"<TargetFrameworks?>([^<]+)<").captures(&t).map(|c| c[1].to_string());
        if let Some(tf) = &tfm {
            notes.push(tf.clone());
            if tf.contains("-windows") {
                hints.push((Os::Windows, 4.0));
            }
        }
        let mut fixed = None;
        if t.contains("<UseWindowsForms>true") {
            notes.push("WinForms".into());
            hints.push((Os::Windows, 4.0));
            fixed = Some("app");
        }
        if t.contains("<UseWPF>true") {
            notes.push("WPF".into());
            hints.push((Os::Windows, 4.0));
            fixed = Some("app");
        }
        if t.contains("Avalonia") {
            notes.push("Avalonia (portable UI)".into());
            fixed = Some("app");
        }
        if t.contains("<UseMaui>true") || t.contains("Microsoft.Maui") {
            notes.push("MAUI (no Linux desktop)".into());
            fixed = Some("app");
        }
        if t.contains("MonoGame") || t.contains("FNA") {
            notes.push("MonoGame/FNA".into());
            fixed = Some("game");
        }
        if t.contains("<OutputType>Library") {
            fixed = fixed.or(Some("library"));
        }
        add(&mut kinds, ".NET", format!("{p} ({})", if notes.is_empty() { "SDK project".into() } else { notes.join(", ") }), fixed);
        for c in re!(r#"<PackageReference\s+Include="([^"]+)""#).captures_iter(&t) {
            push_dep(&mut deps, &c[1], p);
        }
        if name.is_none() {
            name = Path::new(p).file_stem().map(|s| s.to_string_lossy().to_string());
        }
    }
    if let Some(p) = cx.find_name("packages.config") {
        let t = cx.read(p);
        for c in re!(r#"<package\s+id="([^"]+)""#).captures_iter(&t) {
            push_dep(&mut deps, &c[1], p);
        }
    }

    // --- Apple --------------------------------------------------------------------
    if let Some(p) = cx.files.iter().find(|f| f.rel.ends_with(".xcodeproj/project.pbxproj")) {
        let proj = p.rel.trim_end_matches("/project.pbxproj");
        add(&mut kinds, "Xcode", proj.to_string(), None);
        hints.push((Os::Macos, 4.0));
        let t = cx.read(&p.rel);
        let mut seen = BTreeSet::new();
        for c in re!(r"([A-Za-z0-9_]+)\.framework").captures_iter(&t) {
            if seen.insert(c[1].to_string()) {
                push_dep(&mut deps, &format!("{}.framework", &c[1]), &p.rel);
            }
        }
        if name.is_none() {
            name = Path::new(proj).file_stem().map(|s| s.to_string_lossy().to_string());
        }
    }
    if cx.find_name("Info.plist").is_some() {
        hints.push((Os::Macos, 1.5));
    }
    if let Some(p) = cx.find_name("Package.swift") {
        let t = cx.read(p);
        for c in re!(r#"\.package\s*\(\s*(name:\s*"[^"]*",\s*)?url:\s*"[^"]*/([^/"]+?)(\.git)?""#).captures_iter(&t) {
            push_dep(&mut deps, &c[2], p);
        }
    }
    if let Some(p) = cx.find_name("Podfile") {
        let t = cx.read(p);
        for c in re!(r#"(?m)^\s*pod\s+['"]([^'"]+)['"]"#).captures_iter(&t) {
            push_dep(&mut deps, &c[1], p);
        }
        hints.push((Os::Macos, 2.0));
    }

    // --- Rust ------------------------------------------------------------------------
    if let Some(p) = cx.find_name("Cargo.toml") {
        let t = cx.read(p);
        let mut ev = p.to_string();
        let mut fixed = None;
        if let Ok(v) = t.parse::<toml::Table>() {
            if let Some(n) = v.get("package").and_then(|p| p.get("name")).and_then(|n| n.as_str()) {
                name.get_or_insert(n.to_string());
            }
            let mut add_deps = |tbl: Option<&toml::Value>, note: &str| {
                if let Some(tbl) = tbl.and_then(|d| d.as_table()) {
                    for k in tbl.keys() {
                        push_dep_note(&mut deps, k, p, note);
                    }
                }
            };
            add_deps(v.get("dependencies"), "");
            if let Some(targets) = v.get("target").and_then(|t| t.as_table()) {
                for (cfg, body) in targets {
                    add_deps(body.get("dependencies"), &format!("only for {cfg}"));
                }
            }
            if v.get("lib").is_some() && v.get("bin").is_none() && !cx.files.iter().any(|f| f.rel.ends_with("src/main.rs")) {
                fixed = Some("library");
            }
            if v.get("workspace").is_some() {
                ev.push_str(" (workspace)");
            }
        }
        if t.contains("bevy") || t.contains("macroquad") || t.contains("ggez") || t.contains("fyrox") {
            fixed = Some("game");
        }
        add(&mut kinds, "Cargo", ev, fixed);
        if cx.find_name("tauri.conf.json").is_some() {
            add(&mut kinds, "Tauri", cx.find_name("tauri.conf.json").unwrap_or_default().to_string(), Some("app"));
        }
    }
    if let Some(p) = cx.find_name("go.mod") {
        let t = cx.read(p);
        if let Some(c) = re!(r"(?m)^module\s+(\S+)").captures(&t) {
            name.get_or_insert(c[1].rsplit('/').next().unwrap_or(&c[1]).to_string());
        }
        for c in re!(r"(?m)^\s*(?:require\s+)?([a-z0-9.\-]+\.[a-z]+/[^\s]+)\s+v[0-9]").captures_iter(&t) {
            push_dep(&mut deps, &c[1], p);
        }
    }

    // --- Python ------------------------------------------------------------------------
    let py_manifest = ["pyproject.toml", "setup.py", "setup.cfg", "requirements.txt", "Pipfile"]
        .iter()
        .find_map(|n| cx.find_name(n));
    if let Some(p) = py_manifest {
        add(&mut kinds, "Python", p.to_string(), None);
    } else if cx.files.iter().any(|f| f.lang.map(|l| l.id == "python").unwrap_or(false)) {
        let first = cx.files.iter().find(|f| f.lang.map(|l| l.id == "python").unwrap_or(false)).unwrap();
        add(&mut kinds, "Python", format!("{} (scripts, no manifest)", first.rel), None);
    }
    if let Some(p) = cx.find_name("requirements.txt") {
        for line in cx.read(p).lines() {
            let l = line.trim();
            if l.is_empty() || l.starts_with('#') || l.starts_with('-') {
                continue;
            }
            let n: String = l.chars().take_while(|c| c.is_alphanumeric() || "-_.".contains(*c)).collect();
            let note = if l.contains("sys_platform") { "platform-conditional" } else { "" };
            push_dep_note(&mut deps, &n, p, note);
        }
    }
    if let Some(p) = cx.find_name("pyproject.toml") {
        if let Ok(v) = cx.read(p).parse::<toml::Table>() {
            if let Some(n) = v.get("project").and_then(|p| p.get("name")).and_then(|n| n.as_str()) {
                name.get_or_insert(n.to_string());
            }
            if let Some(arr) = v.get("project").and_then(|p| p.get("dependencies")).and_then(|d| d.as_array()) {
                for d in arr.iter().filter_map(|d| d.as_str()) {
                    let n: String = d.chars().take_while(|c| c.is_alphanumeric() || "-_.".contains(*c)).collect();
                    push_dep(&mut deps, &n, p);
                }
            }
            if let Some(t) = v.get("tool").and_then(|t| t.get("poetry")).and_then(|p| p.get("dependencies")).and_then(|d| d.as_table()) {
                for k in t.keys().filter(|k| *k != "python") {
                    push_dep(&mut deps, k, p);
                }
            }
        }
    }

    // --- Node / Electron -----------------------------------------------------------------
    if let Some(p) = cx.find_name("package.json") {
        let t = cx.read(p);
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&t) {
            if let Some(n) = v.get("name").and_then(|n| n.as_str()) {
                name.get_or_insert(n.to_string());
            }
            let mut all = Vec::new();
            for (sec, note) in [("dependencies", ""), ("devDependencies", "dev"), ("optionalDependencies", "optional")] {
                if let Some(o) = v.get(sec).and_then(|d| d.as_object()) {
                    for k in o.keys() {
                        all.push(k.clone());
                        push_dep_note(&mut deps, k, p, note);
                    }
                }
            }
            if all.iter().any(|d| d == "electron") {
                add(&mut kinds, "Electron", format!("{p} (electron dependency)"), Some("app"));
            } else if all.iter().any(|d| d == "react-native") {
                add(&mut kinds, "React Native", p.to_string(), Some("app"));
            } else if all.iter().any(|d| d.starts_with("@tauri-apps")) {
                add(&mut kinds, "Tauri", p.to_string(), Some("app"));
            } else {
                add(&mut kinds, "Node", p.to_string(), None);
            }
        }
    }
    if let Some(p) = cx.find_name("pubspec.yaml") {
        let t = cx.read(p);
        if t.contains("flutter:") {
            add(&mut kinds, "Flutter", p.to_string(), Some("app"));
        }
    }

    // --- Java ---------------------------------------------------------------------------
    for gname in ["build.gradle", "build.gradle.kts"] {
        if let Some(p) = cx.find_name(gname) {
            let t = cx.read(p);
            for c in re!(r#"(?:implementation|api|compileOnly|runtimeOnly)\s*\(?\s*['"]([\w.\-]+):([\w.\-]+)"#).captures_iter(&t) {
                push_dep(&mut deps, &format!("{}:{}", &c[1], &c[2]), p);
            }
            if t.contains("com.android") {
                add(&mut kinds, "Android", p.to_string(), Some("app"));
            }
        }
    }
    if let Some(p) = cx.find_name("pom.xml") {
        let t = cx.read(p);
        for c in re!(r"<dependency>\s*<groupId>([^<]+)</groupId>\s*<artifactId>([^<]+)</artifactId>").captures_iter(&t) {
            push_dep(&mut deps, &format!("{}:{}", &c[1], &c[2]), p);
        }
    }

    // --- C/C++ package managers ----------------------------------------------------------------
    if let Some(p) = cx.find_name("vcpkg.json") {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&cx.read(p)) {
            if let Some(arr) = v.get("dependencies").and_then(|d| d.as_array()) {
                for d in arr {
                    let n = d.as_str().map(str::to_string).or_else(|| d.get("name").and_then(|n| n.as_str()).map(str::to_string));
                    if let Some(n) = n {
                        let note = if d.get("platform").is_some() { "platform-conditional" } else { "" };
                        push_dep_note(&mut deps, &n, p, note);
                    }
                }
            }
        }
    }
    for cname in ["conanfile.txt", "conanfile.py"] {
        if let Some(p) = cx.find_name(cname) {
            let t = cx.read(p);
            for c in re!(r#"(?m)^\s*([A-Za-z0-9_\-.]+)/[0-9][^\s@#]*|requires\s*\(\s*['"]([A-Za-z0-9_\-.]+)/"#).captures_iter(&t) {
                if let Some(m) = c.get(1).or(c.get(2)) {
                    push_dep(&mut deps, m.as_str(), p);
                }
            }
        }
    }
    // Unity package manifest.
    if let Some(p) = cx.files.iter().find(|f| f.rel.ends_with("Packages/manifest.json")) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&cx.read(&p.rel)) {
            if let Some(o) = v.get("dependencies").and_then(|d| d.as_object()) {
                for k in o.keys() {
                    push_dep(&mut deps, k, &p.rel);
                }
            }
        }
    }

    // --- engines -------------------------------------------------------------------------------
    if let Some(p) = cx.files.iter().find(|f| f.rel.ends_with("ProjectSettings/ProjectVersion.txt") || f.rel.ends_with("ProjectSettings/ProjectSettings.asset")) {
        let ver = cx.read(&p.rel).lines().find(|l| l.starts_with("m_EditorVersion:")).map(|l| l.trim_start_matches("m_EditorVersion:").trim().to_string());
        add(&mut kinds, "Unity", format!("{}{}", p.rel, ver.map(|v| format!(" (editor {v})")).unwrap_or_default()), Some("game"));
    }
    if let Some(p) = cx.find_ext("uproject").first() {
        add(&mut kinds, "Unreal", p.to_string(), Some("game"));
        name.get_or_insert(Path::new(p).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default());
    }
    if let Some(p) = cx.find_name("project.godot") {
        let t = cx.read(p);
        let ver = if t.contains("config_version=5") { "Godot 4" } else if t.contains("config_version=4") { "Godot 3" } else { "Godot" };
        if let Some(c) = re!(r#"config/name="([^"]+)""#).captures(&t) {
            name.get_or_insert(c[1].to_string());
        }
        add(&mut kinds, "Godot", format!("{p} ({ver})"), Some("game"));
    }
    if cx.find_name("main.lua").is_some() && (cx.find_name("conf.lua").is_some() || cx.has_api(|f| f.api.starts_with("love."))) {
        add(&mut kinds, "LÖVE (Love2D)", cx.find_name("main.lua").unwrap_or_default().to_string(), Some("game"));
    }
    for p in cx.find_ext("pro") {
        let t = cx.read(p);
        if t.contains("QT") || t.contains("SOURCES") {
            add(&mut kinds, "Qt (qmake)", p.to_string(), Some("app"));
            break;
        }
    }

    // --- frameworks detected from source --------------------------------------------------
    for (api, kind, fl) in [
        ("SDL3", "SDL3", "game"),
        ("SDL2", "SDL2", "game"),
        ("GLFW", "GLFW", "game"),
        ("raylib", "raylib", "game"),
        ("sokol", "sokol", "game"),
        ("SFML", "SFML", "game"),
        ("Allegro", "Allegro", "game"),
        ("bgfx", "bgfx", "game"),
        ("Qt", "Qt", "app"),
        ("wxWidgets", "wxWidgets", "app"),
        ("Dear ImGui", "Dear ImGui", "app"),
        ("GTK", "GTK", "app"),
        ("WinForms", "WinForms", "app"),
        ("WPF", "WPF", "app"),
        ("XNA / MonoGame", "MonoGame/XNA", "game"),
        ("Avalonia", "Avalonia", "app"),
        ("MFC", "MFC", "app"),
    ] {
        if let Some(f) = cx.findings.iter().find(|f| f.api == api) {
            add(&mut kinds, kind, format!("{}:{}", f.file, f.line), Some(fl));
        }
    }

    // --- loose project-file OS hints -----------------------------------------------------
    for f in files {
        let n = file_name(&f.rel).to_ascii_lowercase();
        let e = ext(&f.rel);
        match (n.as_str(), e.as_str()) {
            (_, "bat") | (_, "cmd") | (_, "rc") => hints.push((Os::Windows, 0.5)),
            (_, "desktop") | ("pkgbuild", _) => hints.push((Os::Linux, 1.0)),
            (_, "plist") => hints.push((Os::Macos, 0.5)),
            _ => {}
        }
        if f.rel.starts_with("debian/") && n == "control" {
            hints.push((Os::Linux, 2.0));
        }
        // Prebuilt binary libraries in the tree.
        if matches!(e.as_str(), "dll" | "lib" | "so" | "dylib" | "a") || n.contains(".so.") {
            push_dep_note(&mut deps, file_name(&f.rel), &f.rel, "prebuilt binary in tree");
        }
    }
    for fw in frameworks {
        push_dep_note(&mut deps, file_name(fw), fw, "bundled framework");
    }
    // Libraries linked through #pragma comment(lib, ...)
    static PRAGMA_LIB: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"comment\s*\(\s*lib\s*,\s*"([^"]+)""#).unwrap());
    for f in findings.iter().filter(|f| f.api == "#pragma comment(lib)") {
        if let Some(c) = PRAGMA_LIB.captures(&f.snippet) {
            push_dep_note(&mut deps, c[1].trim_end_matches(".lib"), &format!("{}:{}", f.file, f.line), "#pragma comment(lib)");
        }
    }

    let flavor = overall_flavor(&cx, &kinds);
    for (i, k) in kinds.iter_mut().enumerate() {
        k.flavor = inherent.iter().find(|(j, _)| *j == i).map(|(_, f)| f.to_string()).unwrap_or_else(|| flavor.clone());
    }
    if kinds.is_empty() && !files.is_empty() {
        let lead = files.iter().filter(|f| f.lang.map(|l| l.code).unwrap_or(false)).max_by_key(|f| f.lines);
        if let Some(l) = lead {
            kinds.push(ProjectKind {
                name: format!("{} sources", l.lang.map(|x| x.name).unwrap_or("Plain")),
                flavor: flavor.clone(),
                evidence: "no build system file found".into(),
            });
        }
    }

    Detection { kinds, dependencies: deps, os_hints: hints, name, flavor }
}

/// Decide game / app / library / cli / service from findings and kinds.
fn overall_flavor(cx: &Ctx, kinds: &[ProjectKind]) -> String {
    let game_api = |f: &Finding| {
        // Renderers and gamepads mark games; plain keyboard/console input does not.
        f.category == Category::Graphics && f.severity >= crate::model::Severity::Medium
            || [
                "SDL2", "SDL3", "raylib", "GLFW", "XInput", "XInputGetState", "DirectInput", "XAudio2", "XNA / MonoGame", "sokol", "bgfx",
                "GameController", "linux/joystick.h", "pygame", "Godot C#", "love.system.getOS",
            ]
            .contains(&f.api.as_str())
    };
    if kinds.iter().any(|k| ["Unity", "Unreal", "Godot", "LÖVE (Love2D)"].contains(&k.name.as_str())) || cx.has_api(game_api) {
        return "game".into();
    }
    let ui = cx.has_api(|f| matches!(f.category, Category::Ui | Category::Windowing) && f.severity >= crate::model::Severity::Medium);
    if ui || kinds.iter().any(|k| ["Electron", "Flutter", "Tauri", "Qt", "Qt (qmake)", "Android"].contains(&k.name.as_str())) {
        return "app".into();
    }
    if cx.has_api(|f| ["Windows service", "systemd", "ServiceBase"].contains(&f.api.as_str())) {
        return "service".into();
    }
    let has_entry = cx.files.iter().any(|f| f.has_entry);
    if !has_entry && cx.files.iter().any(|f| f.lang.map(|l| l.code).unwrap_or(false)) {
        return "library".into();
    }
    if has_entry {
        return "cli".into();
    }
    "unknown".into()
}

/// System libraries named in CMake/Make files.
fn system_libs_in(txt: &str) -> Vec<String> {
    static RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\b(d3d9|d3d11|d3d12|dxgi|d3dcompiler|dxguid|user32|gdi32|winmm|ws2_32|xinput|dinput8|dsound|ole32|shell32|comctl32|advapi32|X11|Xrandr|Xi|asound|pulse|udev|rt)\b").unwrap()
    });
    let mut v: Vec<String> = RE.captures_iter(txt).map(|c| c[1].to_string()).collect();
    v.sort();
    v.dedup();
    v
}

fn push_dep(deps: &mut Vec<Dependency>, name: &str, source: &str) {
    push_dep_note(deps, name, source, "");
}

fn push_dep_note(deps: &mut Vec<Dependency>, name: &str, source: &str, extra: &str) {
    let name = name.trim();
    if name.is_empty() || deps.iter().any(|d| d.name.eq_ignore_ascii_case(name)) {
        return;
    }
    let (portable, mut note) = classify_dependency(name);
    if !extra.is_empty() {
        note = if note.is_empty() { extra.to_string() } else { format!("{note} ({extra})") };
    }
    deps.push(Dependency { name: name.to_string(), source: source.to_string(), portable, note });
}

/// Known dependency portability. Patterns ending in '*' are prefixes.
const KNOWN: &[(&str, Option<bool>, &str)] = &[
    // Windows system libraries / SDKs
    ("d3d9", Some(false), "Direct3D 9 — Windows only; SDL_GPU/Vulkan or DXVK/GPTK for translation"),
    ("d3d11", Some(false), "Direct3D 11 — Windows only; SDL_GPU/Vulkan, or DXVK/GPTK"),
    ("d3d12", Some(false), "Direct3D 12 — Windows only; Vulkan/Metal via SDL_GPU, or VKD3D-Proton/GPTK"),
    ("dxgi", Some(false), "DXGI — Windows only"),
    ("d3dcompiler*", Some(false), "Runtime HLSL compiler — precompile with DXC + SPIRV-Cross"),
    ("dxguid", Some(false), "DirectX GUIDs — Windows only"),
    ("xinput*", Some(false), "XInput — use SDL3 gamepad API"),
    ("dinput8", Some(false), "DirectInput — use SDL3 joystick API"),
    ("dsound", Some(false), "DirectSound — use miniaudio/SDL3"),
    ("xaudio2*", Some(false), "XAudio2 — use miniaudio/SDL3 or FAudio"),
    ("winmm", Some(false), "Windows multimedia — use miniaudio + std::chrono"),
    ("user32", Some(false), "Win32 user — SDL3/GLFW for windows & input"),
    ("gdi32", Some(false), "GDI — SDL_Renderer/Skia"),
    ("kernel32", Some(false), "Win32 kernel — std/POSIX equivalents"),
    ("ws2_32", Some(false), "WinSock — BSD sockets on other OSes (cf_platform.h maps)"),
    ("ole32", Some(false), "COM — remove or isolate"),
    ("shell32", Some(false), "Shell API — SDL_OpenURL / std::filesystem"),
    ("comctl32", Some(false), "Common controls — portable UI toolkit"),
    ("advapi32", Some(false), "Registry/security — config files"),
    ("opengl32", Some(true), "OpenGL (Windows import lib) — link OpenGL::GL in CMake"),
    // Linux
    ("x11", Some(false), "X11 — Linux/FreeBSD only; SDL3/GLFW"),
    ("xrandr", Some(false), "X11 RandR — SDL3 display API"),
    ("xi", Some(false), "XInput2 (X11) — SDL3"),
    ("asound", Some(false), "ALSA — Linux only; miniaudio/SDL3"),
    ("alsa", Some(false), "ALSA — Linux only; miniaudio/SDL3"),
    ("pulse*", Some(false), "PulseAudio — Linux/FreeBSD; miniaudio/SDL3"),
    ("libpulse*", Some(false), "PulseAudio — Linux/FreeBSD; miniaudio/SDL3"),
    ("udev", Some(false), "udev — Linux only"),
    ("libudev", Some(false), "udev — Linux only"),
    ("rt", Some(false), "librt — not on macOS (functions live in libSystem)"),
    ("gtk*", Some(false), "GTK — available elsewhere but non-native; consider Qt/wxWidgets"),
    ("wayland*", Some(false), "Wayland — Linux/FreeBSD only"),
    ("dbus*", Some(false), "D-Bus — Linux/FreeBSD desktop IPC"),
    // Apple frameworks
    ("cocoa.framework", Some(false), "AppKit — macOS only"),
    ("appkit.framework", Some(false), "AppKit — macOS only"),
    ("metal.framework", Some(false), "Metal — Apple only; SDL_GPU/Vulkan elsewhere"),
    ("metalkit.framework", Some(false), "MetalKit — Apple only"),
    ("quartzcore.framework", Some(false), "Core Animation — Apple only"),
    ("audiotoolbox.framework", Some(false), "AudioToolbox — miniaudio/SDL3 elsewhere"),
    ("coreaudio.framework", Some(false), "CoreAudio — miniaudio/SDL3 elsewhere"),
    ("avfoundation.framework", Some(false), "AVFoundation — FFmpeg/miniaudio elsewhere"),
    ("iokit.framework", Some(false), "IOKit — Apple only"),
    ("gamecontroller.framework", Some(false), "GameController — SDL3 gamepad API"),
    ("foundation.framework", Some(false), "Foundation — Apple only"),
    ("corefoundation.framework", Some(false), "CoreFoundation — Apple only"),
    ("*.framework", Some(false), "Apple framework — macOS only"),
    // Portable C/C++ libraries
    ("sdl3*", Some(true), "Portable (Windows/macOS/Linux/FreeBSD)"),
    ("sdl2*", Some(true), "Portable; consider SDL3"),
    ("sdl", Some(true), "Portable"),
    ("glfw*", Some(true), "Portable"),
    ("raylib", Some(true), "Portable"),
    ("sfml*", Some(true), "Portable"),
    ("glm", Some(true), "Header-only, portable"),
    ("glad", Some(true), "Portable GL loader"),
    ("glew", Some(true), "Portable GL loader"),
    ("opengl", Some(true), "Portable (deprecated on macOS: max 4.1)"),
    ("vulkan*", Some(true), "Portable (MoltenVK on macOS)"),
    ("volk", Some(true), "Portable Vulkan loader"),
    ("imgui", Some(true), "Portable (pick SDL3/GLFW backends)"),
    ("bgfx", Some(true), "Portable"),
    ("miniaudio", Some(true), "Portable"),
    ("openal*", Some(true), "Portable (OpenAL Soft)"),
    ("fmod*", Some(true), "Portable (no FreeBSD binaries)"),
    ("steamworks*", Some(true), "Windows/macOS/Linux (no FreeBSD)"),
    ("boost*", Some(true), "Portable"),
    ("fmt", Some(true), "Portable"),
    ("spdlog", Some(true), "Portable"),
    ("nlohmann-json", Some(true), "Portable"),
    ("nlohmann_json", Some(true), "Portable"),
    ("zlib", Some(true), "Portable"),
    ("libpng", Some(true), "Portable"),
    ("png", Some(true), "Portable"),
    ("openssl", Some(true), "Portable"),
    ("curl", Some(true), "Portable"),
    ("libcurl", Some(true), "Portable"),
    ("threads", Some(true), "CMake Threads — portable"),
    ("pthread", Some(true), "POSIX threads (MSVC needs std::thread or pthreads4w)"),
    ("m", Some(true), "libm (implicit on macOS/Windows)"),
    ("dl", Some(true), "libdl (in libc on macOS/FreeBSD; Windows uses LoadLibrary)"),
    ("qt*", Some(true), "Portable"),
    ("wxwidgets", Some(true), "Portable"),
    ("freetype", Some(true), "Portable"),
    ("harfbuzz", Some(true), "Portable"),
    ("assimp", Some(true), "Portable"),
    ("bullet3", Some(true), "Portable"),
    ("box2d", Some(true), "Portable"),
    ("enet", Some(true), "Portable"),
    ("protobuf", Some(true), "Portable"),
    ("pkgconfig", Some(true), "Build tool"),
    // .NET
    ("microsoft.windows.compatibility", Some(false), "Windows-only APIs pack"),
    ("microsoft.windowsappsdk", Some(false), "WinUI 3 — Windows only; Uno/Avalonia"),
    ("microsoft.web.webview2", Some(false), "WebView2 — Windows only; use Avalonia/Photino webview"),
    ("sharpdx*", Some(false), "DirectX wrapper — Silk.NET/Veldrid"),
    ("vortice.*", Some(false), "DirectX wrapper — Silk.NET/Veldrid"),
    ("naudio*", Some(false), "Windows audio — Silk.NET.OpenAL/ManagedBass"),
    ("system.management", Some(false), "WMI — Windows only"),
    ("microsoft.win32.registry", Some(false), "Registry — config files"),
    ("system.drawing.common", Some(false), "Windows-only since .NET 6 — SkiaSharp/ImageSharp"),
    ("avalonia*", Some(true), "Portable UI"),
    ("skiasharp*", Some(true), "Portable"),
    ("silk.net*", Some(true), "Portable"),
    ("monogame.framework.desktopgl", Some(true), "Portable MonoGame backend"),
    ("monogame.framework.windowsdx", Some(false), "DirectX backend — switch to DesktopGL"),
    ("newtonsoft.json", Some(true), "Portable"),
    ("system.text.json", Some(true), "Portable"),
    ("microsoft.extensions.*", Some(true), "Portable"),
    ("microsoft.maui*", None, "MAUI — Windows/macOS only (no Linux desktop)"),
    // Python
    ("pywin32", Some(false), "Windows only — psutil/pyperclip/pynput alternatives"),
    ("pypiwin32", Some(false), "Windows only"),
    ("wmi", Some(false), "Windows only — psutil"),
    ("comtypes", Some(false), "Windows only"),
    ("pyobjc*", Some(false), "macOS only"),
    ("python-xlib", Some(false), "X11 only"),
    ("pygame*", Some(true), "Portable"),
    ("pyside6", Some(true), "Portable"),
    ("pyqt5", Some(true), "Portable"),
    ("pyqt6", Some(true), "Portable"),
    ("kivy", Some(true), "Portable"),
    ("numpy", Some(true), "Portable"),
    ("requests", Some(true), "Portable"),
    ("psutil", Some(true), "Portable"),
    ("platformdirs", Some(true), "Portable"),
    ("pyinstaller", Some(true), "Builds per-OS bundles (run on each OS)"),
    // Rust
    ("winapi", Some(false), "Windows only — gate with cfg(windows)"),
    ("windows", Some(false), "Windows only — gate with cfg(windows)"),
    ("windows-sys", Some(false), "Windows only — gate with cfg(windows)"),
    ("winreg", Some(false), "Windows only — directories/confy"),
    ("cocoa", Some(false), "macOS only"),
    ("objc", Some(false), "macOS only"),
    ("objc2*", Some(false), "Apple only"),
    ("core-foundation*", Some(false), "Apple only"),
    ("metal", Some(false), "Apple only — wgpu"),
    ("x11", Some(false), "X11 only — winit"),
    ("x11rb", Some(false), "X11 only — winit"),
    ("alsa", Some(false), "Linux only — cpal"),
    ("nix", Some(false), "Unix only"),
    ("winit", Some(true), "Portable"),
    ("wgpu", Some(true), "Portable"),
    ("cpal", Some(true), "Portable"),
    ("rodio", Some(true), "Portable"),
    ("sdl2", Some(true), "Portable"),
    ("sdl3", Some(true), "Portable"),
    ("bevy", Some(true), "Portable (FreeBSD community-supported)"),
    ("tokio", Some(true), "Portable"),
    ("serde*", Some(true), "Portable"),
    ("libc", Some(true), "Portable bindings (APIs vary per OS)"),
    ("dirs*", Some(true), "Portable"),
    ("directories", Some(true), "Portable"),
    ("tauri*", Some(true), "Portable (no FreeBSD bundles)"),
    // Go
    ("golang.org/x/sys", Some(true), "Portable module (windows/unix subpackages are OS-specific)"),
    ("github.com/lxn/walk", Some(false), "Windows only — Fyne/Gio/Wails"),
    ("github.com/lxn/win", Some(false), "Windows only"),
    ("fyne.io/fyne*", Some(true), "Portable"),
    ("github.com/hajimehoshi/ebiten*", Some(true), "Portable"),
    // Node
    ("electron", Some(true), "Portable (no official FreeBSD builds)"),
    ("winreg", Some(false), "Windows only"),
    ("regedit", Some(false), "Windows only"),
    ("edge-js", Some(false), "Windows-centric .NET bridge"),
    ("node-windows", Some(false), "Windows only"),
    ("ffi-napi", None, "Native addon — prebuilt per OS"),
    ("koffi", Some(true), "Portable FFI (load per-OS libraries)"),
    ("node-gyp", None, "Native build — compile per OS"),
    // Unity
    ("com.unity.*", Some(true), "Unity package (portable across Unity desktop targets)"),
];

/// Portability of a dependency by name (None = unknown).
pub fn classify_dependency(name: &str) -> (Option<bool>, String) {
    let lower = name.to_ascii_lowercase();
    let base = lower.trim_end_matches(".lib").trim_end_matches(".dll");
    for (pat, portable, note) in KNOWN {
        let hit = if let Some(prefix) = pat.strip_suffix('*') {
            base.starts_with(prefix)
        } else if let Some(suffix) = pat.strip_prefix('*') {
            base.ends_with(suffix)
        } else {
            base == *pat
        };
        if hit {
            return (*portable, note.to_string());
        }
    }
    if lower.ends_with(".dll") {
        return (Some(false), "Windows DLL — need .so/.dylib builds for other targets".into());
    }
    if lower.ends_with(".dylib") {
        return (Some(false), "macOS dylib — need .dll/.so builds for other targets".into());
    }
    if lower.ends_with(".so") || lower.contains(".so.") {
        return (Some(false), "ELF shared object — need .dll/.dylib builds for other targets".into());
    }
    (None, String::new())
}
