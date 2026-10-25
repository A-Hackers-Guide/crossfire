//! Text templates for generated build, CI and packaging files.
//!
//! Templates use `%%KEY%%` placeholders (so `${...}` in CMake/YAML/shell
//! needs no escaping) and are filled by `render`.

use crate::model::Os;
use std::collections::BTreeSet;
use std::fmt::Write as _;

/// SDL3 tag fetched by the generated CMake when no system SDL3 is found.
pub const SDL3_TAG: &str = "release-3.4.16";

pub fn render(tpl: &str, vars: &[(&str, &str)]) -> String {
    let mut s = tpl.to_string();
    for (k, v) in vars {
        s = s.replace(&format!("%%{k}%%"), v);
    }
    s
}

/// What kind of build the converted project uses (drives CI + build).
#[derive(Debug, Clone, PartialEq)]
pub enum BuildKind {
    CMake,
    Meson,
    Make,
    Cargo,
    DotNet { project: String },
    Go,
    Python { entry: String },
    Node,
    Engine(String),
    Other,
}

/// Per-OS link requirements derived from the APIs a target's sources use.
#[derive(Debug, Default, Clone)]
pub struct LinkSet {
    /// CMake statements run before linking (find_package / pkg_check_modules).
    pub find: BTreeSet<String>,
    /// Items for target_link_libraries.
    pub libs: BTreeSet<String>,
    /// Meson dependency() names.
    pub meson: BTreeSet<String>,
    /// CI package names (apt on Linux, pkg on FreeBSD).
    pub packages: BTreeSet<String>,
}

/// Map API names found in a target's sources to link requirements for `os`.
pub fn link_set(os: Os, apis: &BTreeSet<String>) -> LinkSet {
    let mut ls = LinkSet::default();
    let has = |names: &[&str]| names.iter().any(|n| apis.contains(*n));
    let lib = |ls: &mut LinkSet, l: &str| {
        ls.libs.insert(l.to_string());
    };
    match os {
        Os::Windows => {
            for l in ["user32", "gdi32", "shell32", "ws2_32"] {
                lib(&mut ls, l);
            }
            let map: &[(&[&str], &[&str])] = &[
                (&["Direct3D 11", "D3D11CreateDevice"], &["d3d11", "dxgi"]),
                (&["Direct3D 12", "D3D12CreateDevice"], &["d3d12", "dxgi"]),
                (&["Direct3D 10"], &["d3d10", "dxgi"]),
                (&["Direct3D 9", "Direct3DCreate9"], &["d3d9"]),
                (&["DXGI", "CreateSwapChain"], &["dxgi"]),
                (&["D3DCompile"], &["d3dcompiler"]),
                (&["DirectX/D3DX interfaces", "dxguid"], &["dxguid"]),
                (&["XInput", "XInputGetState"], &["xinput"]),
                (&["DirectInput"], &["dinput8", "dxguid"]),
                (&["DirectSound"], &["dsound", "dxguid"]),
                (&["XAudio2"], &["xaudio2"]),
                (&["mmsystem.h", "timeGetTime", "waveOut", "PlaySound", "midiOut"], &["winmm"]),
                (&["CoInitialize", "ComPtr", "WASAPI", "Media Foundation"], &["ole32", "oleaut32", "uuid"]),
                (&["Media Foundation"], &["mfplat", "mfuuid", "mfreadwrite"]),
                (&["RegOpenKeyEx", "RegQueryValueEx", "RegCloseKey"], &["advapi32"]),
                (&["GetOpenFileName"], &["comdlg32"]),
                (&["commctrl.h"], &["comctl32"]),
                (&["dbghelp.h", "SetUnhandledExceptionFilter"], &["dbghelp"]),
                (&["wininet.h"], &["wininet", "winhttp"]),
                (&["WGL", "OpenGL"], &["opengl32"]),
                (&["timeGetTime"], &["winmm"]),
                (&["Windows HID"], &["hid", "setupapi"]),
                (&["Direct2D/DirectWrite"], &["d2d1", "dwrite", "windowscodecs"]),
            ];
            for (keys, libs) in map {
                if has(keys) {
                    for l in *libs {
                        lib(&mut ls, l);
                    }
                }
            }
        }
        Os::Macos => {
            let map: &[(&[&str], &str)] = &[
                (&["Cocoa/AppKit", "NSApplication", "NSWindow", "NSView", "NSEvent", "NSAlert", "NSMenu", "AppKit controls", "NSImage", "NSWorkspace", "NSPasteboard"], "Cocoa"),
                (&["Metal", "MTLCreateSystemDefaultDevice"], "Metal"),
                (&["MetalKit"], "MetalKit"),
                (&["QuartzCore", "CAMetalLayer"], "QuartzCore"),
                (&["CoreAudio"], "CoreAudio"),
                (&["AudioToolbox", "AudioUnit"], "AudioToolbox"),
                (&["AVFoundation"], "AVFoundation"),
                (&["GameController"], "GameController"),
                (&["IOKit", "IOKit HID"], "IOKit"),
                (&["CoreFoundation", "NSBundle", "CFRunLoop", "NSUserDefaults"], "CoreFoundation"),
                (&["Foundation", "NSString", "NSFileManager", "NSThread", "NSTimer", "NSNotificationCenter"], "Foundation"),
                (&["CVDisplayLink"], "CoreVideo"),
                (&["Security.framework"], "Security"),
                (&["CoreGraphics", "ImageIO"], "CoreGraphics"),
                (&["CoreText"], "CoreText"),
                (&["Carbon"], "Carbon"),
                (&["CoreMIDI"], "CoreMIDI"),
                (&["Accelerate"], "Accelerate"),
                (&["WebKit"], "WebKit"),
                (&["UserNotifications"], "UserNotifications"),
            ];
            for (keys, fw) in map {
                if has(keys) {
                    lib(&mut ls, &format!("\"-framework {fw}\""));
                }
            }
            if has(&["OpenGL", "CGL/NSOpenGL", "Fixed-function GL"]) {
                ls.find.insert("find_package(OpenGL REQUIRED)".into());
                lib(&mut ls, "OpenGL::GL");
            }
        }
        Os::Linux | Os::Freebsd => {
            let x11 = has(&["Xlib", "XOpenDisplay", "XCreateWindow", "XNextEvent", "XEvent", "Xlib drawing", "XShm/Xrandr/Xinput2", "GLX", "xkbcommon"]);
            if x11 {
                ls.find.insert("find_package(X11 REQUIRED)".into());
                lib(&mut ls, "X11::X11");
                ls.meson.insert("x11".into());
                ls.packages.insert(if os == Os::Linux { "libx11-dev" } else { "libX11" }.into());
            }
            if has(&["XShm/Xrandr/Xinput2"]) {
                for l in ["X11::Xext", "X11::Xrandr", "X11::Xi"] {
                    lib(&mut ls, l);
                }
                ls.packages.extend(if os == Os::Linux {
                    ["libxext-dev", "libxrandr-dev", "libxi-dev"].map(String::from).to_vec()
                } else {
                    ["libXext", "libXrandr", "libXi"].map(String::from).to_vec()
                });
            }
            if has(&["xcb"]) {
                lib(&mut ls, "X11::xcb");
                ls.find.insert("find_package(X11 REQUIRED)".into());
                ls.packages.insert(if os == Os::Linux { "libxcb1-dev" } else { "libxcb" }.into());
            }
            if has(&["ALSA", "snd_pcm_open", "snd_mixer"]) {
                ls.find.insert("find_package(ALSA REQUIRED)".into());
                lib(&mut ls, "ALSA::ALSA");
                ls.meson.insert("alsa".into());
                ls.packages.insert(if os == Os::Linux { "libasound2-dev" } else { "alsa-lib" }.into());
            }
            let pkgcfg = |ls: &mut LinkSet, var: &str, module: &str, apt: &str, bsd: &str| {
                ls.find.insert("find_package(PkgConfig REQUIRED)".into());
                ls.find.insert(format!("pkg_check_modules({var} REQUIRED IMPORTED_TARGET {module})"));
                ls.libs.insert(format!("PkgConfig::{var}"));
                ls.meson.insert(module.into());
                ls.packages.insert(if os == Os::Linux { apt } else { bsd }.into());
            };
            if has(&["PulseAudio"]) {
                pkgcfg(&mut ls, "CF_PULSE", "libpulse-simple", "libpulse-dev", "pulseaudio");
            }
            if has(&["PipeWire"]) {
                pkgcfg(&mut ls, "CF_PIPEWIRE", "libpipewire-0.3", "libpipewire-0.3-dev", "pipewire");
            }
            if has(&["GTK"]) {
                pkgcfg(&mut ls, "CF_GTK", "gtk+-3.0", "libgtk-3-dev", "gtk3");
            }
            if has(&["udev"]) {
                pkgcfg(&mut ls, "CF_UDEV", "libudev", "libudev-dev", "libudev-devd");
            }
            if has(&["D-Bus"]) {
                pkgcfg(&mut ls, "CF_DBUS", "dbus-1", "libdbus-1-dev", "dbus");
            }
            if has(&["Wayland"]) {
                pkgcfg(&mut ls, "CF_WAYLAND", "wayland-client", "libwayland-dev", "wayland");
            }
            if has(&["libinput"]) {
                pkgcfg(&mut ls, "CF_LIBINPUT", "libinput", "libinput-dev", "libinput");
            }
            if has(&["systemd"]) && os == Os::Linux {
                pkgcfg(&mut ls, "CF_SYSTEMD", "libsystemd", "libsystemd-dev", "");
            }
            if has(&["OpenGL", "GLX", "Fixed-function GL"]) {
                ls.find.insert("find_package(OpenGL REQUIRED)".into());
                lib(&mut ls, "OpenGL::GL");
                ls.meson.insert("gl".into());
                ls.packages.insert(if os == Os::Linux { "libgl1-mesa-dev" } else { "mesa-libs" }.into());
            }
            if has(&["EGL"]) {
                lib(&mut ls, "EGL");
            }
            if has(&["Vulkan"]) {
                ls.find.insert("find_package(Vulkan REQUIRED)".into());
                lib(&mut ls, "Vulkan::Vulkan");
                ls.packages.insert(if os == Os::Linux { "libvulkan-dev" } else { "vulkan-loader" }.into());
            }
            if os == Os::Freebsd && has(&["backtrace()"]) {
                lib(&mut ls, "execinfo");
            }
            if os == Os::Linux && has(&["clock_gettime", "shm_open", "mmap"]) {
                lib(&mut ls, "rt");
            }
            lib(&mut ls, "m");
        }
    }
    if os != Os::Linux && os != Os::Freebsd && has(&["Vulkan"]) {
        ls.find.insert("find_package(Vulkan REQUIRED)".into());
        ls.libs.insert("Vulkan::Vulkan".into());
    }
    ls
}

/// Inputs for the generated CMake/Meson project.
#[derive(Debug, Clone)]
pub struct NativePlan {
    pub name: String,
    pub display_name: String,
    pub bundle_id: String,
    pub common_sources: Vec<String>,
    /// Sources that only compile on some OSes, with the stub used elsewhere.
    pub affine: Vec<AffineSource>,
    pub include_dirs: Vec<String>,
    pub executable: bool,
    pub win32_subsystem: bool,
    pub gui: bool,
    pub has_objc: bool,
    pub cxx_standard: String,
    pub sdl3_default: bool,
    pub macos_arch: String,
    pub links: Vec<(Os, LinkSet)>,
    pub packages: Vec<String>,
    /// Sources excluded because they define a second entry point.
    pub excluded: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct AffineSource {
    pub path: String,
    pub oses: Vec<Os>,
    pub stub: Option<String>,
}

pub fn cmake_condition(oses: &[Os]) -> String {
    let parts: Vec<&str> = oses
        .iter()
        .map(|o| match o {
            Os::Windows => "WIN32",
            Os::Macos => "APPLE",
            Os::Linux => "CF_LINUX",
            Os::Freebsd => "CF_FREEBSD",
        })
        .collect();
    parts.join(" OR ")
}

fn cmake_os_var(os: Os) -> &'static str {
    match os {
        Os::Windows => "WIN32",
        Os::Macos => "APPLE",
        Os::Linux => "CF_LINUX",
        Os::Freebsd => "CF_FREEBSD",
    }
}

/// Body shared by the generated CMakeLists.txt and crossfire/crossfire.cmake:
/// per-OS sources, link libraries and settings for `${tgt}`.
fn cmake_target_setup(p: &NativePlan, tgt: &str, existing: bool, base: &str) -> String {
    let mut s = String::new();
    if !p.affine.is_empty() {
        s.push_str("\n# Platform-specific sources. On other OSes a generated stub is compiled instead\n# so the project links; the stubs are listed in CROSSFIRE_REPORT.md.\n");
        for a in &p.affine {
            let cond = cmake_condition(&a.oses);
            if existing {
                let _ = writeln!(s, "if(NOT ({cond}))");
                let _ = writeln!(s, "  set_source_files_properties({base}/{} PROPERTIES HEADER_FILE_ONLY ON)", a.path);
                if let Some(stub) = &a.stub {
                    let _ = writeln!(s, "  target_sources({tgt} PRIVATE {base}/{stub})");
                }
                s.push_str("endif()\n");
            } else {
                let _ = writeln!(s, "if({cond})");
                let _ = writeln!(s, "  target_sources({tgt} PRIVATE {})", a.path);
                if let Some(stub) = &a.stub {
                    s.push_str("else()\n");
                    let _ = writeln!(s, "  target_sources({tgt} PRIVATE {stub})");
                }
                s.push_str("endif()\n");
            }
        }
    }
    let _ = writeln!(s, "\nfind_package(Threads REQUIRED)\ntarget_link_libraries({tgt} PRIVATE Threads::Threads ${{CMAKE_DL_LIBS}})");
    s.push_str("\nif(CF_USE_SDL3)\n");
    let _ = writeln!(s, "  target_link_libraries({tgt} PRIVATE SDL3::SDL3)\n  target_compile_definitions({tgt} PRIVATE CF_USE_SDL3=1)");
    s.push_str("endif()\n\n");
    let mut first = true;
    for os in [Os::Windows, Os::Macos, Os::Linux, Os::Freebsd] {
        let ls = p.links.iter().find(|(o, _)| *o == os).map(|(_, l)| l.clone()).unwrap_or_default();
        let kw = if first { "if" } else { "elseif" };
        first = false;
        let _ = writeln!(s, "{kw}({})", cmake_os_var(os));
        match os {
            Os::Windows => {
                let _ = writeln!(s, "  target_compile_definitions({tgt} PRIVATE _CRT_SECURE_NO_WARNINGS _WINSOCK_DEPRECATED_NO_WARNINGS)");
                let _ = writeln!(s, "  if(MSVC)\n    target_compile_options({tgt} PRIVATE /utf-8 /W3)\n  endif()");
            }
            Os::Macos => {
                if !existing && p.gui && p.executable {
                    let _ = writeln!(
                        s,
                        "  set_target_properties({tgt} PROPERTIES\n    MACOSX_BUNDLE_GUI_IDENTIFIER \"{}\"\n    MACOSX_BUNDLE_BUNDLE_NAME \"{}\"\n    MACOSX_BUNDLE_BUNDLE_VERSION \"${{PROJECT_VERSION}}\"\n    MACOSX_BUNDLE_SHORT_VERSION_STRING \"${{PROJECT_VERSION}}\"\n    MACOSX_BUNDLE_INFO_PLIST \"${{CMAKE_CURRENT_SOURCE_DIR}}/packaging/macos/Info.plist.in\")",
                        p.bundle_id, p.display_name
                    );
                }
            }
            Os::Freebsd => {
                let _ = writeln!(s, "  # FreeBSD ports/packages install into /usr/local.");
                let _ = writeln!(s, "  target_include_directories({tgt} SYSTEM PRIVATE /usr/local/include)");
                let _ = writeln!(s, "  target_link_directories({tgt} PRIVATE /usr/local/lib)");
            }
            Os::Linux => {}
        }
        for f in &ls.find {
            let _ = writeln!(s, "  {f}");
        }
        if !ls.libs.is_empty() {
            let libs: Vec<&str> = ls.libs.iter().map(String::as_str).collect();
            let _ = writeln!(s, "  target_link_libraries({tgt} PRIVATE {})", libs.join(" "));
        }
    }
    s.push_str("endif()\n");
    s
}

/// Generated top-level CMakeLists.txt.
pub fn cmake_lists(p: &NativePlan) -> String {
    let tgt = &p.name;
    let mut s = String::new();
    let _ = writeln!(s, "# Generated by Crossfire: cross-platform build for {} (Windows, macOS, Linux, FreeBSD).", p.display_name);
    s.push_str("cmake_minimum_required(VERSION 3.21)\n\n");
    let arch = match p.macos_arch.as_str() {
        "arm64" => "arm64",
        "x86_64" => "x86_64",
        _ => "arm64;x86_64",
    };
    let _ = writeln!(
        s,
        "# macOS architecture and deployment target must be set before project().\nif(APPLE AND NOT CMAKE_OSX_ARCHITECTURES)\n  set(CMAKE_OSX_ARCHITECTURES \"{arch}\" CACHE STRING \"macOS architectures\")\nendif()\nset(CMAKE_OSX_DEPLOYMENT_TARGET \"11.0\" CACHE STRING \"Minimum macOS version\")\n"
    );
    let _ = writeln!(s, "project({tgt} VERSION 1.0.0 LANGUAGES C CXX)");
    if p.has_objc {
        s.push_str("if(APPLE)\n  enable_language(OBJC OBJCXX)\nendif()\n");
    }
    let _ = writeln!(
        s,
        "\nset(CMAKE_C_STANDARD 11)\nset(CMAKE_C_EXTENSIONS ON)\nset(CMAKE_CXX_STANDARD {})\nset(CMAKE_CXX_STANDARD_REQUIRED ON)\nset(CMAKE_EXPORT_COMPILE_COMMANDS ON)\nif(NOT CMAKE_BUILD_TYPE AND NOT CMAKE_CONFIGURATION_TYPES)\n  set(CMAKE_BUILD_TYPE Release)\nendif()\n",
        p.cxx_standard
    );
    s.push_str("if(CMAKE_SYSTEM_NAME STREQUAL \"Linux\")\n  set(CF_LINUX ON)\nelseif(CMAKE_SYSTEM_NAME MATCHES \"FreeBSD|DragonFly\")\n  set(CF_FREEBSD ON)\nendif()\n\n");
    s.push_str(&sdl3_block(p.sdl3_default));
    s.push_str("\nset(CF_SOURCES\n");
    for src in &p.common_sources {
        let _ = writeln!(s, "  {src}");
    }
    s.push_str("  crossfire/cf_platform.c\n)\n");
    if !p.excluded.is_empty() {
        let _ = writeln!(s, "# Not compiled (additional entry points): {}", p.excluded.join(", "));
    }
    if p.executable {
        let mut flags = String::new();
        if p.win32_subsystem {
            flags.push_str(" WIN32");
        }
        if p.gui {
            flags.push_str(" MACOSX_BUNDLE");
        }
        let _ = writeln!(s, "add_executable({tgt}{flags} ${{CF_SOURCES}})");
    } else {
        let _ = writeln!(s, "add_library({tgt} ${{CF_SOURCES}})");
    }
    let mut incs = vec!["${CMAKE_CURRENT_SOURCE_DIR}".to_string()];
    incs.extend(p.include_dirs.iter().filter(|d| !d.is_empty()).map(|d| format!("${{CMAKE_CURRENT_SOURCE_DIR}}/{d}")));
    let _ = writeln!(s, "target_include_directories({tgt} PRIVATE\n  {}\n)", incs.join("\n  "));
    s.push_str(&cmake_target_setup(p, tgt, false, "${CMAKE_CURRENT_SOURCE_DIR}"));

    // Install + CPack packaging.
    let _ = writeln!(
        s,
        "\ninclude(GNUInstallDirs)\ninstall(TARGETS {tgt}\n  RUNTIME DESTINATION ${{CMAKE_INSTALL_BINDIR}}\n  LIBRARY DESTINATION ${{CMAKE_INSTALL_LIBDIR}}\n  ARCHIVE DESTINATION ${{CMAKE_INSTALL_LIBDIR}}{}\n)",
        if p.gui && p.executable { "\n  BUNDLE DESTINATION ." } else { "" }
    );
    if p.packages.iter().any(|x| x == "appimage") {
        let _ = writeln!(
            s,
            "if(CF_LINUX OR CF_FREEBSD)\n  install(FILES packaging/linux/{tgt}.desktop DESTINATION ${{CMAKE_INSTALL_DATADIR}}/applications)\n  install(FILES packaging/linux/{tgt}.svg DESTINATION ${{CMAKE_INSTALL_DATADIR}}/icons/hicolor/scalable/apps)\nendif()"
        );
    }
    let nsis = p.packages.iter().any(|x| x == "nsis");
    let _ = writeln!(
        s,
        "\nset(CPACK_PACKAGE_NAME \"{tgt}\")\nset(CPACK_PACKAGE_VENDOR \"{tgt}\")\nset(CPACK_PACKAGE_DESCRIPTION_SUMMARY \"{} (ported with Crossfire)\")\nset(CPACK_PACKAGE_DIRECTORY \"${{CMAKE_BINARY_DIR}}/packages\")\nif(WIN32)\n  set(CPACK_GENERATOR \"ZIP{}\")\nelseif(APPLE)\n  set(CPACK_GENERATOR \"DragNDrop\")\nelseif(CF_FREEBSD)\n  set(CPACK_GENERATOR \"TGZ\")\n  # For a native .pkg run packaging/freebsd/make-pkg.sh (or add FREEBSD here).\n  set(CPACK_FREEBSD_PACKAGE_MAINTAINER \"you@example.com\")\n  set(CPACK_FREEBSD_PACKAGE_ORIGIN \"misc/{tgt}\")\n  set(CPACK_FREEBSD_PACKAGE_LICENSE \"BSD2CLAUSE\")\nelse()\n  set(CPACK_GENERATOR \"TGZ\")\nendif()\ninclude(CPack)",
        p.display_name,
        if nsis { ";NSIS" } else { "" }
    );
    s
}

fn sdl3_block(default_on: bool) -> String {
    format!(
        r#"# SDL3 portability layer: a system SDL3 is used when available, otherwise it is fetched.
option(CF_USE_SDL3 "Build with SDL3 (windowing, input, audio, dialogs)" {on})
if(CF_USE_SDL3)
  # Package-manager SDL3 builds (Homebrew, MacPorts) are single-architecture,
  # so universal macOS builds always compile their own static SDL3.
  if(NOT (APPLE AND CMAKE_OSX_ARCHITECTURES MATCHES ";"))
    find_package(SDL3 CONFIG QUIET)
  endif()
  if(NOT SDL3_FOUND)
    include(FetchContent)
    set(SDL_SHARED OFF CACHE BOOL "" FORCE)
    set(SDL_STATIC ON CACHE BOOL "" FORCE)
    set(SDL_TEST_LIBRARY OFF CACHE BOOL "" FORCE)
    FetchContent_Declare(SDL3
      GIT_REPOSITORY https://github.com/libsdl-org/SDL.git
      GIT_TAG {tag}
      GIT_SHALLOW TRUE)
    FetchContent_MakeAvailable(SDL3)
  endif()
endif()
"#,
        on = if default_on { "ON" } else { "OFF" },
        tag = SDL3_TAG
    )
}

/// crossfire/crossfire.cmake for projects that keep their own CMakeLists.
pub fn crossfire_cmake(p: &NativePlan) -> String {
    let mut s = String::new();
    s.push_str("# Generated by Crossfire. Include from your CMakeLists.txt:\n#   include(crossfire/crossfire.cmake)\n#   crossfire_configure(<your-target>)\n");
    s.push_str("if(CMAKE_SYSTEM_NAME STREQUAL \"Linux\")\n  set(CF_LINUX ON)\nelseif(CMAKE_SYSTEM_NAME MATCHES \"FreeBSD|DragonFly\")\n  set(CF_FREEBSD ON)\nendif()\n\n");
    s.push_str(&sdl3_block(p.sdl3_default));
    s.push_str("\nmacro(crossfire_configure tgt)\n");
    s.push_str("  target_sources(${tgt} PRIVATE ${CMAKE_CURRENT_LIST_DIR}/crossfire/cf_platform.c)\n");
    s.push_str("  target_include_directories(${tgt} PRIVATE ${CMAKE_CURRENT_LIST_DIR})\n");
    let body = cmake_target_setup(p, "${tgt}", true, "${CMAKE_CURRENT_LIST_DIR}");
    for line in body.lines() {
        if line.is_empty() {
            s.push('\n');
        } else {
            let _ = writeln!(s, "  {line}");
        }
    }
    s.push_str("endmacro()\n");
    s
}

pub fn cmake_presets(p: &NativePlan) -> String {
    let arch = match p.macos_arch.as_str() {
        "arm64" => "arm64",
        "x86_64" => "x86_64",
        _ => "arm64;x86_64",
    };
    let cond = |host: &str| serde_json::json!({"type": "equals", "lhs": "${hostSystemName}", "rhs": host});
    let presets = serde_json::json!({
        "version": 6,
        "cmakeMinimumRequired": {"major": 3, "minor": 21, "patch": 0},
        "configurePresets": [
            {"name": "default", "displayName": "Release (host)", "binaryDir": "${sourceDir}/build/${presetName}",
             "cacheVariables": {"CMAKE_BUILD_TYPE": "Release"}},
            {"name": "debug", "displayName": "Debug (host)", "inherits": "default",
             "cacheVariables": {"CMAKE_BUILD_TYPE": "Debug"}},
            {"name": "windows", "displayName": "Windows (Visual Studio 2022)", "inherits": "default",
             "generator": "Visual Studio 17 2022", "architecture": "x64", "condition": cond("Windows")},
            {"name": "macos", "displayName": "macOS", "inherits": "default", "condition": cond("Darwin"),
             "cacheVariables": {"CMAKE_OSX_ARCHITECTURES": arch, "CMAKE_OSX_DEPLOYMENT_TARGET": "11.0"}},
            {"name": "linux", "displayName": "Linux", "inherits": "default", "condition": cond("Linux")},
            {"name": "freebsd", "displayName": "FreeBSD", "inherits": "default", "condition": cond("FreeBSD"),
             "cacheVariables": {"CMAKE_PREFIX_PATH": "/usr/local"}}
        ],
        "buildPresets": [
            {"name": "default", "configurePreset": "default", "configuration": "Release"},
            {"name": "debug", "configurePreset": "debug", "configuration": "Debug"},
            {"name": "windows", "configurePreset": "windows", "configuration": "Release"},
            {"name": "macos", "configurePreset": "macos"},
            {"name": "linux", "configurePreset": "linux"},
            {"name": "freebsd", "configurePreset": "freebsd"}
        ]
    });
    serde_json::to_string_pretty(&presets).unwrap_or_default() + "\n"
}

pub fn meson_build(p: &NativePlan) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "# Generated by Crossfire.\nproject('{}', ['c', 'cpp'], version: '1.0.0',\n  default_options: ['c_std=gnu11', 'cpp_std=c++{}', 'buildtype=release'])\n", p.name, p.cxx_standard);
    s.push_str("cc = meson.get_compiler('c')\nsys = host_machine.system()\nsrcs = files(\n");
    for src in &p.common_sources {
        let _ = writeln!(s, "  '{src}',");
    }
    s.push_str("  'crossfire/cf_platform.c',\n)\ndeps = [dependency('threads'), cc.find_library('dl', required: false), cc.find_library('m', required: false)]\n");
    for a in &p.affine {
        let conds: Vec<String> = a
            .oses
            .iter()
            .map(|o| format!("sys == '{}'", match o { Os::Windows => "windows", Os::Macos => "darwin", Os::Linux => "linux", Os::Freebsd => "freebsd" }))
            .collect();
        let _ = writeln!(s, "if {}\n  srcs += files('{}')", conds.join(" or "), a.path);
        if let Some(stub) = &a.stub {
            let _ = writeln!(s, "else\n  srcs += files('{stub}')");
        }
        s.push_str("endif\n");
    }
    for (os, ls) in &p.links {
        if ls.meson.is_empty() {
            continue;
        }
        let sysname = match os { Os::Windows => "windows", Os::Macos => "darwin", Os::Linux => "linux", Os::Freebsd => "freebsd" };
        let _ = writeln!(s, "if sys == '{sysname}'");
        for d in &ls.meson {
            let _ = writeln!(s, "  deps += dependency('{d}')");
        }
        s.push_str("endif\n");
    }
    let mut incs = vec!["'.'".to_string()];
    incs.extend(p.include_dirs.iter().map(|d| format!("'{d}'")));
    let _ = writeln!(
        s,
        "{}('{}', srcs, dependencies: deps, include_directories: include_directories({}), install: true)",
        if p.executable { "executable" } else { "library" },
        p.name,
        incs.join(", ")
    );
    s
}

// ---------------------------------------------------------------------------
// CI workflow
// ---------------------------------------------------------------------------

pub struct CiPlan<'a> {
    pub name: &'a str,
    pub kind: &'a BuildKind,
    pub targets: &'a [Os],
    pub apt: Vec<String>,
    pub pkg: Vec<String>,
    pub sdl3: bool,
    pub packages: &'a [String],
}

const SDL3_APT: &str = "libx11-dev libxext-dev libxrandr-dev libxcursor-dev libxi-dev libxss-dev libxkbcommon-dev libxtst-dev libwayland-dev libegl1-mesa-dev libgl1-mesa-dev libasound2-dev libpulse-dev libdbus-1-dev libudev-dev libdrm-dev libgbm-dev";

pub fn workflow(c: &CiPlan) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "# Generated by Crossfire: builds {} on every target OS.", c.name);
    s.push_str("name: Crossfire build\n\non:\n  push:\n  pull_request:\n  workflow_dispatch:\n\njobs:\n");
    let runners: Vec<(Os, &str)> = c
        .targets
        .iter()
        .filter_map(|t| match t {
            Os::Windows => Some((*t, "windows-latest")),
            Os::Macos => Some((*t, "macos-latest")),
            Os::Linux => Some((*t, "ubuntu-latest")),
            Os::Freebsd => None,
        })
        .collect();
    let (setup, build, artifact) = ci_steps(c);
    if !runners.is_empty() {
        s.push_str("  build:\n    name: ${{ matrix.target }}\n    strategy:\n      fail-fast: false\n      matrix:\n        include:\n");
        for (os, runner) in &runners {
            let _ = writeln!(s, "          - target: {}\n            runner: {runner}", os.id());
        }
        s.push_str("    runs-on: ${{ matrix.runner }}\n    steps:\n      - uses: actions/checkout@v4\n");
        let mut apt: Vec<String> = c.apt.clone();
        if c.sdl3 {
            apt.extend(SDL3_APT.split(' ').map(String::from));
        }
        if matches!(c.kind, BuildKind::CMake | BuildKind::Meson) {
            apt.push("ninja-build".into());
        }
        if matches!(c.kind, BuildKind::Meson) {
            apt.push("meson".into());
        }
        apt.sort();
        apt.dedup();
        if !apt.is_empty() && c.targets.contains(&Os::Linux) {
            let _ = writeln!(s, "      - name: Install Linux dependencies\n        if: runner.os == 'Linux'\n        run: sudo apt-get update && sudo apt-get install -y {}", apt.join(" "));
        }
        s.push_str(&setup);
        s.push_str(&build);
        if c.packages.iter().any(|p| p == "appimage") && matches!(c.kind, BuildKind::CMake) {
            s.push_str("      - name: AppImage\n        if: runner.os == 'Linux'\n        run: bash packaging/linux/build-appimage.sh build/ci\n");
        }
        if c.packages.iter().any(|p| p == "dmg" || p == "app") && matches!(c.kind, BuildKind::CMake) {
            s.push_str("      - name: macOS bundle\n        if: runner.os == 'macOS'\n        run: bash packaging/macos/make-app.sh build/ci\n");
        }
        let _ = writeln!(s, "      - uses: actions/upload-artifact@v4\n        with:\n          name: {}-${{{{ matrix.target }}}}\n          path: {artifact}\n          if-no-files-found: warn", c.name);
    }
    if c.targets.contains(&Os::Freebsd) {
        let mut pkgs: Vec<String> = c.pkg.clone();
        pkgs.extend(freebsd_tool_pkgs(c.kind).into_iter().map(String::from));
        if c.sdl3 {
            pkgs.push("sdl3".into());
        }
        pkgs.sort();
        pkgs.dedup();
        let run = freebsd_run(c.kind, c.name);
        let _ = writeln!(
            s,
            "  freebsd:\n    name: freebsd\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@v4\n      - name: Build in FreeBSD VM\n        uses: vmactions/freebsd-vm@v1\n        with:\n          usesh: true\n          prepare: pkg install -y {}\n          run: |\n{}      - uses: actions/upload-artifact@v4\n        with:\n          name: {}-freebsd\n          path: {artifact}\n          if-no-files-found: warn",
            pkgs.join(" "),
            run.lines().map(|l| format!("            {l}\n")).collect::<String>(),
            c.name
        );
    }
    s
}

fn freebsd_tool_pkgs(kind: &BuildKind) -> Vec<&'static str> {
    match kind {
        BuildKind::CMake => vec!["cmake", "ninja", "pkgconf", "git"],
        BuildKind::Meson => vec!["meson", "ninja", "pkgconf"],
        BuildKind::Make => vec!["gmake", "pkgconf"],
        BuildKind::Cargo => vec!["rust", "pkgconf"],
        BuildKind::Go => vec!["go"],
        BuildKind::Python { .. } => vec!["python3", "py311-pip"],
        BuildKind::Node => vec!["node", "npm"],
        BuildKind::DotNet { .. } => vec!["dotnet"],
        _ => vec!["git"],
    }
}

fn freebsd_run(kind: &BuildKind, name: &str) -> String {
    match kind {
        BuildKind::CMake => "cmake -S . -B build/ci -G Ninja -DCMAKE_BUILD_TYPE=Release\ncmake --build build/ci --parallel\n(cd build/ci && cpack) || true\n".into(),
        BuildKind::Meson => "meson setup build/ci --buildtype=release\nmeson compile -C build/ci\n".into(),
        BuildKind::Make => "gmake -j4\n".into(),
        BuildKind::Cargo => "cargo build --release\n".into(),
        BuildKind::Go => format!("mkdir -p build/ci && go build -o build/ci/{name} .\n"),
        BuildKind::Python { entry } => format!("python3 -m pip install --user pyinstaller -r requirements.txt || python3 -m pip install --user pyinstaller\npython3 -m PyInstaller --onefile --distpath build/ci {entry}\n"),
        BuildKind::Node => "npm ci\nnpm run build --if-present\n".into(),
        BuildKind::DotNet { project } => format!("dotnet publish {project} -c Release -o build/ci\n"),
        _ => "echo 'No FreeBSD build steps for this project type'\n".into(),
    }
}

/// (setup steps, build steps, artifact path) for the matrix job.
fn ci_steps(c: &CiPlan) -> (String, String, String) {
    match c.kind {
        BuildKind::CMake => (
            String::new(),
            "      - name: Configure\n        run: cmake -S . -B build/ci -DCMAKE_BUILD_TYPE=Release\n      - name: Build\n        run: cmake --build build/ci --config Release --parallel\n      - name: Package (CPack)\n        run: cpack --config build/ci/CPackConfig.cmake -C Release -B build/ci/packages\n        continue-on-error: true\n".into(),
            "build/ci/packages/*".into(),
        ),
        BuildKind::Meson => (
            "      - uses: actions/setup-python@v5\n        with:\n          python-version: '3.12'\n      - run: pip install meson ninja\n".into(),
            "      - run: meson setup build/ci --buildtype=release\n      - run: meson compile -C build/ci\n".into(),
            "build/ci".into(),
        ),
        BuildKind::Make => (
            String::new(),
            "      - name: Build\n        shell: bash\n        run: make -j4\n".into(),
            ".".into(),
        ),
        BuildKind::Cargo => (
            "      - uses: dtolnay/rust-toolchain@stable\n      - uses: Swatinem/rust-cache@v2\n".into(),
            "      - run: cargo build --release\n".into(),
            "target/release".into(),
        ),
        BuildKind::Go => (
            "      - uses: actions/setup-go@v5\n        with:\n          go-version: stable\n".into(),
            format!("      - shell: bash\n        run: mkdir -p build/ci && go build -o build/ci/{}$([ \"$RUNNER_OS\" = Windows ] && echo .exe) .\n", c.name),
            "build/ci".into(),
        ),
        BuildKind::DotNet { project } => (
            "      - uses: actions/setup-dotnet@v4\n        with:\n          dotnet-version: '8.0.x'\n".into(),
            format!(
                "      - name: Publish\n        shell: bash\n        run: |\n          case \"$RUNNER_OS\" in\n            Windows) RID=win-x64 ;;\n            macOS) RID=osx-arm64 ;;\n            *) RID=linux-x64 ;;\n          esac\n          dotnet publish {project} -c Release -r $RID --self-contained true -p:EnableWindowsTargeting=true -o build/ci\n"
            ),
            "build/ci".into(),
        ),
        BuildKind::Python { entry } => (
            "      - uses: actions/setup-python@v5\n        with:\n          python-version: '3.12'\n".into(),
            format!("      - shell: bash\n        run: |\n          pip install pyinstaller\n          if [ -f requirements.txt ]; then pip install -r requirements.txt; fi\n          pyinstaller --onefile --distpath build/ci {entry}\n"),
            "build/ci".into(),
        ),
        BuildKind::Node => (
            "      - uses: actions/setup-node@v4\n        with:\n          node-version: 22\n".into(),
            "      - run: npm ci\n      - run: npm run build --if-present\n      - run: npx --yes electron-builder --publish never || true\n        shell: bash\n".into(),
            "dist".into(),
        ),
        BuildKind::Engine(e) => (
            String::new(),
            format!("      - run: echo \"{e} projects are exported from the editor (see game-ci / godot-ci actions).\"\n"),
            ".".into(),
        ),
        BuildKind::Other => (String::new(), "      - run: echo \"Add build steps here\"\n".into(), ".".into()),
    }
}

// ---------------------------------------------------------------------------
// Packaging
// ---------------------------------------------------------------------------

pub const INFO_PLIST_IN: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<!-- Generated by Crossfire. CMake fills in the MACOSX_BUNDLE_ variables below. -->
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key><string>en</string>
  <key>CFBundleExecutable</key><string>${MACOSX_BUNDLE_EXECUTABLE_NAME}</string>
  <key>CFBundleIdentifier</key><string>${MACOSX_BUNDLE_GUI_IDENTIFIER}</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>CFBundleName</key><string>${MACOSX_BUNDLE_BUNDLE_NAME}</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>${MACOSX_BUNDLE_SHORT_VERSION_STRING}</string>
  <key>CFBundleVersion</key><string>${MACOSX_BUNDLE_BUNDLE_VERSION}</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSSupportsAutomaticGraphicsSwitching</key><true/>
  <key>LSApplicationCategoryType</key><string>%%CATEGORY%%</string>
</dict>
</plist>
"#;

pub const INFO_PLIST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<!-- Generated by Crossfire for script-built bundles (packaging/macos/make-app.sh). -->
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key><string>en</string>
  <key>CFBundleExecutable</key><string>%%NAME%%</string>
  <key>CFBundleIdentifier</key><string>%%BUNDLE_ID%%</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>CFBundleName</key><string>%%DISPLAY%%</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>1.0.0</string>
  <key>CFBundleVersion</key><string>1</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>LSApplicationCategoryType</key><string>%%CATEGORY%%</string>
</dict>
</plist>
"#;

pub const MAKE_APP_SH: &str = r#"#!/usr/bin/env bash
# Generated by Crossfire: assemble %%DISPLAY%%.app (if the build did not) and a .dmg.
# Usage: packaging/macos/make-app.sh [build-dir]   (env: BIN, CODESIGN_IDENTITY, MAKE_DMG=0)
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BUILD="${1:-$ROOT/build/macos}"
APP="%%NAME%%"
BUNDLE="$(find "$BUILD" -maxdepth 4 -name "$APP.app" -type d | head -n 1 || true)"
if [ -z "$BUNDLE" ]; then
  BIN="${BIN:-$BUILD/$APP}"
  BUNDLE="$BUILD/$APP.app"
  mkdir -p "$BUNDLE/Contents/MacOS" "$BUNDLE/Contents/Resources"
  cp "$BIN" "$BUNDLE/Contents/MacOS/$APP"
  cp "$ROOT/packaging/macos/Info.plist" "$BUNDLE/Contents/Info.plist"
fi
# Ad-hoc signature by default; set CODESIGN_IDENTITY="Developer ID Application: ..." to sign for distribution.
codesign --force --deep --sign "${CODESIGN_IDENTITY:--}" "$BUNDLE"
if [ "${MAKE_DMG:-1}" = "1" ]; then
  hdiutil create -volname "%%DISPLAY%%" -srcfolder "$BUNDLE" -ov -format UDZO "$BUILD/$APP.dmg"
  echo "Created $BUILD/$APP.dmg"
fi
"#;

pub const APPIMAGE_SH: &str = r#"#!/usr/bin/env bash
# Generated by Crossfire: build %%NAME%%-x86_64.AppImage with linuxdeploy.
# Usage: packaging/linux/build-appimage.sh [build-dir]   (env: BIN, LINUXDEPLOY)
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BUILD="${1:-$ROOT/build/linux}"
APP="%%NAME%%"
APPDIR="$BUILD/AppDir"
rm -rf "$APPDIR"
if [ -f "$BUILD/cmake_install.cmake" ]; then
  DESTDIR="$APPDIR" cmake --install "$BUILD" --prefix /usr
fi
if [ ! -x "$APPDIR/usr/bin/$APP" ]; then
  mkdir -p "$APPDIR/usr/bin"
  cp "${BIN:-$BUILD/$APP}" "$APPDIR/usr/bin/$APP"
fi
LINUXDEPLOY="${LINUXDEPLOY:-$(command -v linuxdeploy || true)}"
if [ -z "$LINUXDEPLOY" ]; then
  LINUXDEPLOY="$BUILD/linuxdeploy-$(uname -m).AppImage"
  curl -fsSL -o "$LINUXDEPLOY" "https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-$(uname -m).AppImage"
  chmod +x "$LINUXDEPLOY"
fi
export APPIMAGE_EXTRACT_AND_RUN=1
cd "$BUILD"
"$LINUXDEPLOY" --appdir "$APPDIR" \
  --executable "$APPDIR/usr/bin/$APP" \
  --desktop-file "$ROOT/packaging/linux/$APP.desktop" \
  --icon-file "$ROOT/packaging/linux/$APP.svg" \
  --output appimage
"#;

pub const DESKTOP: &str = "[Desktop Entry]\nType=Application\nName=%%DISPLAY%%\nExec=%%NAME%%\nIcon=%%NAME%%\nCategories=%%DESKTOP_CATEGORY%%;\nTerminal=%%TERMINAL%%\n";

pub const ICON_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="256" height="256" viewBox="0 0 256 256">
  <!-- Placeholder icon generated by Crossfire; replace with your own artwork. -->
  <rect width="256" height="256" rx="48" fill="#1f2937"/>
  <path d="M128 40c18 34 54 50 54 98a54 54 0 0 1-108 0c0-28 16-42 26-60 4 18 12 26 22 30-4-26 0-48 6-68z" fill="#f97316"/>
</svg>
"##;

pub const PACKAGE_ZIP_PS1: &str = r#"# Generated by Crossfire: zip the Windows build of %%NAME%%.
# Usage: pwsh packaging/windows/package-zip.ps1 [-BuildDir build\windows]
param([string]$BuildDir = "build\windows")
$ErrorActionPreference = "Stop"
$Root = Resolve-Path "$PSScriptRoot\..\.."
$Stage = Join-Path $BuildDir "stage"
if (Test-Path $Stage) { Remove-Item -Recurse -Force $Stage }
cmake --install $BuildDir --prefix $Stage --config Release
$Zip = Join-Path $BuildDir "%%NAME%%-windows-x64.zip"
Compress-Archive -Path "$Stage\*" -DestinationPath $Zip -Force
Write-Host "Created $Zip"
"#;

pub const INSTALLER_NSI: &str = r#"; Generated by Crossfire: NSIS installer for %%DISPLAY%%.
; Build: makensis /DSTAGE=build\windows\stage packaging\windows\installer.nsi
!ifndef STAGE
  !define STAGE "..\..\build\windows\stage"
!endif
Name "%%DISPLAY%%"
OutFile "%%NAME%%-setup.exe"
InstallDir "$PROGRAMFILES64\%%DISPLAY%%"
RequestExecutionLevel admin
Unicode true

Section "Install"
  SetOutPath "$INSTDIR"
  File /r "${STAGE}\*.*"
  CreateDirectory "$SMPROGRAMS\%%DISPLAY%%"
  CreateShortcut "$SMPROGRAMS\%%DISPLAY%%\%%DISPLAY%%.lnk" "$INSTDIR\bin\%%NAME%%.exe"
  WriteUninstaller "$INSTDIR\uninstall.exe"
SectionEnd

Section "Uninstall"
  RMDir /r "$INSTDIR"
  RMDir /r "$SMPROGRAMS\%%DISPLAY%%"
SectionEnd
"#;

pub const FREEBSD_MANIFEST: &str = r#"name: "%%NAME%%"
version: "1.0.0"
origin: "misc/%%NAME%%"
comment: "%%DISPLAY%% (ported with Crossfire)"
desc: "%%DISPLAY%% built for FreeBSD."
maintainer: "you@example.com"
www: "https://example.com"
prefix: "/usr/local"
"#;

pub const FREEBSD_PKG_SH: &str = r#"#!/bin/sh
# Generated by Crossfire: build a FreeBSD package (.pkg) for %%NAME%%.
# Usage: packaging/freebsd/make-pkg.sh [build-dir]   (run on FreeBSD)
set -eu
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BUILD="${1:-$ROOT/build/freebsd}"
STAGE="$BUILD/stage"
rm -rf "$STAGE"
mkdir -p "$STAGE"
DESTDIR="$STAGE" cmake --install "$BUILD" --prefix /usr/local
(cd "$STAGE" && find . -type f | sed 's#^\./#/#') > "$BUILD/pkg-plist"
pkg create -M "$ROOT/packaging/freebsd/+MANIFEST" -r "$STAGE" -p "$BUILD/pkg-plist" -o "$BUILD"
echo "Package written to $BUILD"
"#;
