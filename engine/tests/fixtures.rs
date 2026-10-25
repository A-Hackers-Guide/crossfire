//! Scan + convert the fixture projects end to end.

use crossfire_engine::*;
use std::path::{Path, PathBuf};
use std::process::Command;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

fn scan_fixture(name: &str) -> ScanReport {
    let r = scan(&fixture(name), &|_| {}).expect("scan");
    // Must round-trip through JSON (the UI contract).
    let json = serde_json::to_string(&r).unwrap();
    let back: ScanReport = serde_json::from_str(&json).unwrap();
    assert_eq!(back.findings.len(), r.findings.len());
    r
}

fn convert_fixture(name: &str, targets: &[Os], answers: serde_json::Value) -> (tempfile::TempDir, ConvertResult) {
    let out = tempfile::tempdir().unwrap();
    let req = ConvertRequest {
        source: fixture(name).to_string_lossy().into(),
        output: out.path().join("out").to_string_lossy().into(),
        source_os: vec![],
        targets: targets.to_vec(),
        answers: serde_json::from_value(answers).unwrap(),
        ai: AiSettings::default(),
    };
    let events = std::cell::RefCell::new(0usize);
    let res = convert(&req, &|_| *events.borrow_mut() += 1).expect("convert");
    assert!(*events.borrow() > 5, "progress events emitted");
    let json = serde_json::to_string(&res).unwrap();
    let _: ConvertResult = serde_json::from_str(&json).unwrap();
    (out, res)
}

fn change<'a>(r: &'a ConvertResult, path: &str) -> &'a FileChange {
    r.changes.iter().find(|c| c.path == path).unwrap_or_else(|| panic!("no change for {path}"))
}

fn read(r: &ConvertResult, rel: &str) -> String {
    std::fs::read_to_string(Path::new(&r.output).join(rel)).unwrap_or_else(|_| panic!("missing {rel}"))
}

fn source_unchanged(name: &str) {
    // The fixtures must never be modified by convert.
    let main = fixture(name);
    assert!(!main.join("crossfire").exists(), "convert wrote into the source tree");
    assert!(!main.join("CROSSFIRE_REPORT.md").exists());
}

#[test]
fn win32_directx_game() {
    let r = scan_fixture("win32_dx_game");
    assert_eq!(r.detected_os, vec![Os::Windows]);
    assert!(r.kinds.iter().any(|k| k.name == "MSBuild"));
    assert!(r.kinds.iter().all(|k| k.flavor == "game"), "{:?}", r.kinds);
    for api in ["Direct3D 11", "XInput", "CreateWindowEx", "Sleep", "QueryPerformanceCounter", "RegOpenKeyEx", "HLSL", "Batch script"] {
        assert!(r.findings.iter().any(|f| f.api == api), "missing {api}");
    }
    assert!(r.portability < 60.0, "{}", r.portability);
    assert!(r.dependencies.iter().any(|d| d.name == "d3d11" && d.portable == Some(false)));
    let qs = questions(&r, &[Os::Linux, Os::Macos, Os::Freebsd]);
    let ids: Vec<&str> = qs.iter().map(|q| q.id.as_str()).collect();
    for id in ["strategy", "graphics", "shaders", "audio", "windowing", "input", "buildSystem", "cppStandard", "ci", "packages", "useAi", "keepOriginals"] {
        if id == "audio" {
            continue; // no audio APIs in this fixture
        }
        assert!(ids.contains(&id), "missing question {id}: {ids:?}");
    }
    assert_eq!(qs.iter().find(|q| q.id == "shaders").unwrap().depends_on.as_ref().unwrap().question, "graphics");

    let (_tmp, res) = convert_fixture("win32_dx_game", &[Os::Windows, Os::Macos, Os::Linux, Os::Freebsd], serde_json::json!({}));
    source_unchanged("win32_dx_game");
    let game = read(&res, "src/Game.cpp");
    assert!(game.contains("#include \"Game.h\""), "include case fixed: {game}");
    assert!(game.contains("cf_stricmp("));
    assert!(game.contains("#ifdef _WIN32\n#include <windows.h>\n#endif"));
    let main = read(&res, "src/main.cpp");
    assert!(main.contains("cf::sleep_ms(1)"));
    assert!(main.contains("cf_user_data_dir()"));
    assert!(main.contains("#ifdef _MSC_VER\n#pragma comment(lib, \"d3d11.lib\")"));
    assert!(change(&res, "src/main.cpp").diff.as_ref().unwrap().contains("+#ifdef _WIN32"));
    // main.cpp is Windows-only: compiled on Windows, stub main elsewhere.
    let cmake = read(&res, "CMakeLists.txt");
    assert!(cmake.contains("if(WIN32)\n  target_sources(") && cmake.contains("d3d11"), "{cmake}");
    assert!(read(&res, "crossfire/stubs/src_main.stub.cpp").contains("int main(int argc"));
    for f in ["crossfire/cf_platform.h", "crossfire/cf_platform.c", "CMakePresets.json", ".github/workflows/crossfire-build.yml", "CROSSFIRE_REPORT.md", "packaging/macos/Info.plist.in"] {
        assert!(Path::new(&res.output).join(f).exists(), "{f}");
    }
    let wf = read(&res, ".github/workflows/crossfire-build.yml");
    assert!(wf.contains("vmactions/freebsd-vm") && wf.contains("windows-latest") && wf.contains("macos-latest") && wf.contains("ubuntu-latest"));
    assert!(!res.manual_review.is_empty());
}

#[test]
fn cocoa_metal_app() {
    let r = scan_fixture("cocoa_metal_app");
    assert_eq!(r.detected_os, vec![Os::Macos]);
    assert!(r.kinds.iter().any(|k| k.name == "Xcode"));
    for api in ["Cocoa/AppKit", "MetalKit", "Metal Shading Language", "Grand Central Dispatch", "NSBundle", "AVFoundation", "mach_absolute_time"] {
        assert!(r.findings.iter().any(|f| f.api == api), "missing {api}");
    }
    assert!(r.dependencies.iter().any(|d| d.name == "Metal.framework" && d.portable == Some(false)));
    let (_tmp, res) = convert_fixture("cocoa_metal_app", &[Os::Windows, Os::Linux], serde_json::json!({}));
    let cmake = read(&res, "CMakeLists.txt");
    assert!(cmake.contains("enable_language(OBJC OBJCXX)"));
    assert!(cmake.contains("Sources/geometry.c"), "portable C stays common");
    assert!(cmake.contains("\"-framework Metal\""), "{cmake}");
    assert!(res.changes.iter().any(|c| c.path.starts_with("crossfire/stubs/")), "objc files get stubs");
}

#[test]
fn linux_x11_alsa_converts_and_compiles() {
    let r = scan_fixture("linux_x11_alsa");
    assert_eq!(r.detected_os, vec![Os::Linux, Os::Freebsd], "X11 is shared with FreeBSD");
    assert!(r.kinds.iter().any(|k| k.name == "Make"));
    for api in ["Xlib", "XOpenDisplay", "ALSA", "snd_pcm_open", "/proc/self/exe", "#ifdef __linux__", "sysinfo"] {
        assert!(r.findings.iter().any(|f| f.api == api), "missing {api}");
    }
    let (_tmp, res) = convert_fixture("linux_x11_alsa", &Os::ALL, serde_json::json!({"buildSystem": "cmake"}));
    let main = read(&res, "src/main.c");
    assert!(main.contains("cf_executable_path_into(exe,"), "{main}");
    assert!(main.contains("cf_sleep_us(16000)"));
    assert!(read(&res, "run.sh").starts_with("#!/usr/bin/env bash"));
    let cmake = read(&res, "CMakeLists.txt");
    assert!(cmake.contains("if(CF_LINUX OR CF_FREEBSD)\n  target_sources(linux_x11_alsa PRIVATE src/window_x11.c)"), "{cmake}");
    assert!(cmake.contains("if(CF_LINUX)\n  target_sources(linux_x11_alsa PRIVATE src/audio_alsa.c)"), "{cmake}");
    assert!(cmake.contains("ALSA::ALSA") && cmake.contains("X11::X11"));

    // The generated project must configure and build on this machine
    // (X11/ALSA backends are replaced by the generated stubs off Linux).
    let Some(cmake_bin) = which_cmake() else { return };
    let bdir = Path::new(&res.output).join("build/test");
    let ok = Command::new(&cmake_bin)
        .args(["-S", &res.output, "-B"])
        .arg(&bdir)
        .args(["-DCF_USE_SDL3=OFF", "-DCMAKE_BUILD_TYPE=Release"])
        .status()
        .unwrap()
        .success();
    assert!(ok, "cmake configure failed");
    let ok = Command::new(&cmake_bin).arg("--build").arg(&bdir).arg("--parallel").status().unwrap().success();
    assert!(ok, "cmake build failed");
}

fn which_cmake() -> Option<PathBuf> {
    ["/opt/homebrew/bin/cmake", "/usr/local/bin/cmake", "/usr/bin/cmake"].iter().map(PathBuf::from).find(|p| p.exists())
}

#[test]
fn csharp_winforms_app() {
    let r = scan_fixture("winforms_app");
    assert_eq!(r.detected_os, vec![Os::Windows]);
    let net = r.kinds.iter().find(|k| k.name == ".NET").expect(".NET kind");
    assert!(net.evidence.contains("WinForms") && net.flavor == "app", "{net:?}");
    for api in ["WinForms", "DllImport user32/kernel32", "Microsoft.Win32.Registry", "net*-windows TFM"] {
        assert!(r.findings.iter().any(|f| f.api == api), "missing {api}");
    }
    assert!(r.dependencies.iter().any(|d| d.name == "Microsoft.Win32.Registry" && d.portable == Some(false)));
    let qs = questions(&r, &[Os::Macos, Os::Linux]);
    let ui = qs.iter().find(|q| q.id == "windowing").unwrap();
    assert_eq!(ui.default, serde_json::json!("avalonia"));
    let (_tmp, res) = convert_fixture("winforms_app", &[Os::Macos, Os::Linux], serde_json::json!({}));
    let form = read(&res, "Notes/MainForm.cs");
    assert!(form.contains("Crossfire.CfPlatform.UserDataDir"));
    assert!(form.contains("\"Notes/notes.txt\""));
    assert!(form.contains("Crossfire.CfPlatform.OpenPath("));
    assert!(read(&res, "Notes/Notes.csproj").contains("<EnableWindowsTargeting>true"));
    assert!(Path::new(&res.output).join("Notes/Crossfire/CfPlatform.cs").exists());
    assert!(res.manual_review.iter().any(|f| f.api == "WinForms"));
}

#[test]
fn python_winreg_script() {
    let r = scan_fixture("python_winreg");
    assert_eq!(r.detected_os, vec![Os::Windows]);
    for api in ["winreg", "msvcrt", "os.startfile", "APPDATA env", "os.system('cls')"] {
        assert!(r.findings.iter().any(|f| f.api == api), "missing {api}");
    }
    assert!(r.dependencies.iter().any(|d| d.name.starts_with("pywin32") && d.portable == Some(false)));
    let (tmp, res) = convert_fixture("python_winreg", &[Os::Linux, Os::Macos], serde_json::json!({}));
    let py = read(&res, "settings_tool.py");
    assert!(py.contains("from crossfire import cf_winreg as winreg"));
    assert!(py.contains("from crossfire import cf_msvcrt as msvcrt"));
    assert!(py.contains("cf_platform.open_path(export_dir())"));
    assert!(py.contains("os.path.join(cf_platform.user_data_dir(), \"SettingsTool\")"));
    assert!(py.contains("cf_platform.clear_screen()"));
    assert!(py.contains("\"logs/settings.log\""));

    // Exercise the converted module with the winreg emulation.
    let Some(py3) = ["/usr/bin/python3", "/opt/homebrew/bin/python3"].iter().find(|p| Path::new(p).exists()) else { return };
    let reg = tmp.path().join("reg.json");
    let out = Command::new(py3)
        .current_dir(&res.output)
        .env("CF_REGISTRY_FILE", &reg)
        .args(["-c", "import settings_tool as s; s.save('theme','dark'); print(s.load('theme')); print(s.load('missing','dflt'))"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(String::from_utf8_lossy(&out.stdout), "dark\ndflt\n");
    assert!(reg.exists());
}

#[test]
fn convert_refuses_to_write_into_source() {
    let req = ConvertRequest {
        source: fixture("python_winreg").to_string_lossy().into(),
        output: fixture("python_winreg").to_string_lossy().into(),
        source_os: vec![],
        targets: vec![Os::Linux],
        answers: Default::default(),
        ai: AiSettings::default(),
    };
    assert!(convert(&req, &|_| {}).is_err());
}

#[test]
fn enums_serialize_camel_case() {
    assert_eq!(serde_json::to_string(&Category::DynamicLibrary).unwrap(), "\"dynamicLibrary\"");
    assert_eq!(serde_json::to_string(&Os::Macos).unwrap(), "\"macos\"");
    assert_eq!(serde_json::to_string(&AiProvider::OpenaiCompatible).unwrap(), "\"openaiCompatible\"");
    assert!(rule_count() >= 250);
}
