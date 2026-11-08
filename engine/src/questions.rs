//! Conversion questions tailored to a scan report and target OS set.

use crate::model::*;
use serde_json::{json, Value};

fn opt(value: &str, label: &str, description: &str, recommended: bool) -> QuestionOption {
    QuestionOption {
        value: value.into(),
        label: label.into(),
        description: description.into(),
        recommended: recommended.then_some(true),
    }
}

/// Pick the recommended option's value (or the first) as the default.
fn default_of(options: &[QuestionOption]) -> Value {
    let v = options
        .iter()
        .find(|o| o.recommended == Some(true))
        .or(options.first())
        .map(|o| o.value.clone());
    json!(v.unwrap_or_default())
}

fn single(id: &str, title: &str, description: &str, options: Vec<QuestionOption>) -> Question {
    Question {
        id: id.into(),
        title: title.into(),
        description: description.into(),
        kind: QuestionKind::Single,
        default: default_of(&options),
        options,
        depends_on: None,
    }
}

fn toggle(id: &str, title: &str, description: &str, default: bool) -> Question {
    Question {
        id: id.into(),
        title: title.into(),
        description: description.into(),
        kind: QuestionKind::Toggle,
        options: vec![],
        default: json!(default),
        depends_on: None,
    }
}

/// Coarse facts about the scanned project used to tailor questions.
pub struct Profile {
    pub cfamily: bool,
    pub dotnet: bool,
    pub python: bool,
    pub rust: bool,
    pub go: bool,
    pub swift: bool,
    pub js: bool,
    pub engine: Option<String>,
    pub game: bool,
    pub has_cmake: bool,
    pub has_msbuild: bool,
    pub has_xcode: bool,
    pub has_make: bool,
    pub graphics: bool,
    pub audio: bool,
    pub windowing: bool,
    pub ui: bool,
    pub input: bool,
    pub winforms: bool,
    pub portability: f64,
}

pub fn profile(scan: &ScanReport) -> Profile {
    let lang = |n: &[&str]| {
        scan.languages
            .iter()
            .any(|l| n.contains(&l.name.as_str()) && l.lines > 0)
    };
    let kind = |n: &str| scan.kinds.iter().any(|k| k.name == n);
    let cat = |c: Category| {
        scan.findings
            .iter()
            .any(|f| f.category == c && f.severity >= Severity::Medium && f.os.len() < 4)
    };
    let engine = ["Unity", "Unreal", "Godot", "LÖVE (Love2D)"]
        .iter()
        .find(|e| kind(e))
        .map(|s| s.to_string());
    Profile {
        cfamily: lang(&["C", "C++", "Objective-C", "Objective-C++", "C/C++ Header"]),
        dotnet: lang(&["C#", "F#", "VB.NET"]) || kind(".NET"),
        python: lang(&["Python"]),
        rust: kind("Cargo"),
        go: kind("Go"),
        swift: lang(&["Swift"]),
        js: kind("Node") || kind("Electron") || kind("Tauri"),
        game: scan.kinds.iter().any(|k| k.flavor == "game"),
        engine,
        has_cmake: kind("CMake"),
        has_msbuild: kind("MSBuild"),
        has_xcode: kind("Xcode"),
        has_make: kind("Make"),
        graphics: cat(Category::Graphics),
        audio: cat(Category::Audio),
        windowing: cat(Category::Windowing),
        ui: cat(Category::Ui),
        input: cat(Category::Input),
        winforms: scan.findings.iter().any(|f| {
            f.api == "WinForms" || f.api == "WPF" || f.api == "UseWindowsForms" || f.api == "UseWPF"
        }),
        portability: scan.portability,
    }
}

/// Generate the question list for a scan and a set of target OSes.
pub fn questions(scan: &ScanReport, targets: &[Os]) -> Vec<Question> {
    let p = profile(scan);
    let mut qs = Vec::new();
    let targets: Vec<Os> = if targets.is_empty() {
        Os::ALL.to_vec()
    } else {
        targets.to_vec()
    };
    let new_targets: Vec<Os> = targets
        .iter()
        .copied()
        .filter(|t| !scan.detected_os.contains(t))
        .collect();
    let tlabel = new_targets
        .iter()
        .map(|o| o.label())
        .collect::<Vec<_>>()
        .join(", ");
    let to_mac = targets.contains(&Os::Macos);

    // 1. Overall strategy -------------------------------------------------------------
    let native_heavy = p.graphics || p.windowing || p.ui || p.audio;
    if let Some(engine) = &p.engine {
        qs.push(single(
            "strategy",
            "Porting strategy",
            &format!("{engine} already targets several OSes; Crossfire will fix platform-conditional code and native plugins."),
            vec![
                opt("engine", &format!("Use {engine}'s exporters"), "Fix #if branches, native plugins and paths; build with the engine's export targets.", true),
                opt("translation", "Translation layer", "Ship the Windows build and run it via Wine/Proton, GPTK or CrossOver.", false),
            ],
        ));
    } else if p.dotnet {
        qs.push(single(
            "strategy",
            "Porting strategy",
            "How should the .NET code reach the new platforms?",
            vec![
                opt("framework", "Cross-platform .NET libraries", "Keep C#, swap Windows-only libraries (WinForms/WPF → Avalonia, Registry → settings file, System.Drawing → SkiaSharp).", true),
                opt("hybrid", "Hybrid", "Shared core library + per-OS UI heads.", false),
                opt("translation", "Translation layer", "Run the Windows build under Wine/CrossOver/GPTK (WinForms works reasonably on Wine).", false),
            ],
        ));
    } else {
        let translation_rec = p.portability < 15.0 && p.game && !p.cfamily;
        qs.push(single(
            "strategy",
            "Porting strategy",
            &format!(
                "Portability is {:.0}%. {}",
                scan.portability,
                if tlabel.is_empty() { "Choose how platform-specific code is handled.".to_string() } else { format!("How should the code reach {tlabel}?") }
            ),
            vec![
                opt("sdl3", "Portability layer (SDL3)", "Replace OS APIs with SDL3 + portable libraries (std::filesystem, miniaudio...). One code path for all OSes.", native_heavy && !translation_rec),
                opt("native", "Native backend per OS", "Keep a platform interface and write Win32 / Cocoa / X11-Wayland backends. Most native feel, most work.", false),
                opt("translation", "Translation layer", "Ship as-is and rely on Wine/Proton + DXVK (Linux/FreeBSD), GPTK/CrossOver (macOS), MoltenVK. Fastest, least native.", translation_rec),
                opt("hybrid", "Hybrid", "Portability layer for most code, native backends where it matters (e.g. Metal renderer on macOS).", false),
                opt("minimal", "Minimal (POSIX/C++ standard only)", "Only fix OS-level calls (time, files, threads); best for CLI tools and libraries.", !native_heavy),
            ],
        ));
    }

    // 2. Graphics -----------------------------------------------------------------------
    if p.graphics && p.engine.is_none() {
        let opts = if p.dotnet {
            vec![
                opt(
                    "silk",
                    "Silk.NET (Vulkan/OpenGL)",
                    "Low-level bindings available on every desktop OS.",
                    true,
                ),
                opt(
                    "veldrid",
                    "Veldrid",
                    "Abstracts D3D11/Vulkan/Metal/OpenGL.",
                    false,
                ),
                opt(
                    "monogame",
                    "MonoGame DesktopGL / FNA",
                    "For XNA-style games.",
                    false,
                ),
                opt(
                    "keep",
                    "Keep current API",
                    "Rely on a translation layer for graphics.",
                    false,
                ),
            ]
        } else if p.rust {
            vec![
                opt(
                    "wgpu",
                    "wgpu",
                    "WebGPU API over Vulkan/Metal/D3D12/GL.",
                    true,
                ),
                opt(
                    "ash",
                    "Vulkan (ash)",
                    "Raw Vulkan everywhere (MoltenVK on macOS).",
                    false,
                ),
                opt(
                    "keep",
                    "Keep current API",
                    "Gate per-OS renderers with cfg.",
                    false,
                ),
            ]
        } else {
            vec![
                opt("sdlgpu", "SDL_GPU", "SDL3's modern GPU API: Vulkan, Metal and D3D12 backends from one code path.", true),
                opt("vulkan", "Vulkan (+MoltenVK)", "Explicit API everywhere; MoltenVK on macOS.", false),
                opt("opengl", "OpenGL 3.3/4.1 core", "Simplest for small renderers; deprecated on macOS (max 4.1).", false),
                opt("bgfx", "bgfx", "Mature cross-platform rendering library.", false),
                opt("webgpu", "WebGPU (Dawn/wgpu-native)", "Modern portable API, also runs in browsers.", false),
                opt("keep", "Keep per-OS renderers", "Keep the existing renderer for its OS and add one per new OS (e.g. Metal-cpp).", false),
            ]
        };
        qs.push(single(
            "graphics",
            "Graphics backend",
            "Rendering API to port the renderer to.",
            opts,
        ));
        if p.cfamily {
            let mut q = single(
                "shaders",
                "Shader pipeline",
                "How shaders are produced for each backend.",
                vec![
                    opt(
                        "shadercross",
                        "SDL_shadercross (HLSL → SPIR-V/MSL/DXIL)",
                        "Keep HLSL as the source language and generate the rest offline.",
                        true,
                    ),
                    opt(
                        "slang",
                        "Slang",
                        "Single-source shading language targeting every backend.",
                        false,
                    ),
                    opt(
                        "glsl",
                        "GLSL + SPIRV-Cross",
                        "Author GLSL/SPIR-V and cross-compile to MSL/HLSL.",
                        false,
                    ),
                ],
            );
            q.depends_on = Some(DependsOn {
                question: "graphics".into(),
                value: "sdlgpu".into(),
            });
            qs.push(q);
        }
    }

    // 3. Audio ------------------------------------------------------------------------------
    if p.audio && p.engine.is_none() {
        let opts = if p.dotnet {
            vec![
                opt(
                    "openal",
                    "OpenAL (Silk.NET / OpenTK)",
                    "Portable 3D audio.",
                    true,
                ),
                opt(
                    "bass",
                    "ManagedBass",
                    "BASS bindings (commercial license).",
                    false,
                ),
                opt("keep", "Keep current API", "Windows-only audio.", false),
            ]
        } else if p.python {
            vec![
                opt(
                    "sounddevice",
                    "sounddevice / simpleaudio",
                    "PortAudio-backed playback on every OS.",
                    true,
                ),
                opt(
                    "pygame",
                    "pygame.mixer",
                    "If the app already uses pygame.",
                    false,
                ),
            ]
        } else {
            vec![
                opt(
                    "miniaudio",
                    "miniaudio",
                    "Single header; WASAPI, CoreAudio, ALSA, PulseAudio, PipeWire, OSS, sndio.",
                    true,
                ),
                opt(
                    "sdl3",
                    "SDL3 audio streams",
                    "Natural choice if SDL3 is used for windowing.",
                    false,
                ),
                opt("openal", "OpenAL Soft", "3D positional audio API.", false),
                opt(
                    "fmod",
                    "FMOD / Wwise",
                    "Middleware (no FreeBSD binaries).",
                    false,
                ),
                opt(
                    "keep",
                    "Keep current API",
                    "Keep native audio per OS.",
                    false,
                ),
            ]
        };
        qs.push(single(
            "audio",
            "Audio backend",
            "Replacement for platform audio APIs.",
            opts,
        ));
    }

    // 4. Windowing / UI -----------------------------------------------------------------------
    if (p.windowing || p.ui) && p.engine.is_none() {
        let opts = if p.dotnet {
            vec![
                opt(
                    "avalonia",
                    "Avalonia UI",
                    "WPF-like XAML, runs on Windows/macOS/Linux (FreeBSD community).",
                    true,
                ),
                opt(
                    "maui",
                    ".NET MAUI",
                    "Windows + macOS only (no Linux desktop).",
                    false,
                ),
                opt(
                    "eto",
                    "Eto.Forms",
                    "WinForms-like API over native toolkits.",
                    false,
                ),
                opt(
                    "keep",
                    "Keep WinForms/WPF",
                    "Windows-only UI; use Wine elsewhere.",
                    false,
                ),
            ]
        } else if p.python {
            vec![
                opt(
                    "pyside6",
                    "PySide6 (Qt)",
                    "Native-looking UI on every OS.",
                    true,
                ),
                opt(
                    "tkinter",
                    "tkinter",
                    "Ships with Python, basic look.",
                    false,
                ),
                opt("keep", "Keep current toolkit", "", false),
            ]
        } else if p.rust {
            vec![
                opt(
                    "winit",
                    "winit (+egui/iced)",
                    "Portable windowing; pick an immediate- or retained-mode UI.",
                    true,
                ),
                opt("sdl3", "SDL3 (sdl3 crate)", "", false),
                opt("keep", "Keep per-OS code", "", false),
            ]
        } else if p.game {
            vec![
                opt(
                    "sdl3",
                    "SDL3",
                    "Windows, input, dialogs, tray and clipboard on all four OSes.",
                    true,
                ),
                opt(
                    "glfw",
                    "GLFW",
                    "Lightweight window + input for GL/Vulkan.",
                    false,
                ),
                opt(
                    "native",
                    "Native per OS",
                    "Win32 / Cocoa / X11+Wayland backends.",
                    false,
                ),
            ]
        } else {
            vec![
                opt(
                    "qt",
                    "Qt 6",
                    "Full native-looking widget toolkit on all four OSes.",
                    true,
                ),
                opt(
                    "wxwidgets",
                    "wxWidgets",
                    "Wraps native widgets (Win32, Cocoa, GTK).",
                    false,
                ),
                opt(
                    "imgui",
                    "Dear ImGui + SDL3",
                    "Immediate-mode UI, ideal for tools.",
                    false,
                ),
                opt(
                    "sdl3",
                    "SDL3 only",
                    "Plain windows + SDL dialogs/message boxes.",
                    false,
                ),
                opt(
                    "native",
                    "Native per OS",
                    "Keep native UI code per platform.",
                    false,
                ),
            ]
        };
        qs.push(single(
            "windowing",
            "Windowing & UI toolkit",
            "Replacement for native windows, menus and dialogs.",
            opts,
        ));
    }

    // 5. Input ----------------------------------------------------------------------------------
    if p.input && p.engine.is_none() && !p.dotnet && !p.python {
        qs.push(single(
            "input",
            "Input & gamepads",
            "XInput / DirectInput / GameController / evdev replacement.",
            vec![
                opt(
                    "sdl3",
                    "SDL3 gamepad API",
                    "Unified mapping for Xbox, PlayStation, Switch and generic controllers.",
                    true,
                ),
                opt(
                    "glfw",
                    "GLFW gamepad API",
                    "Uses SDL_GameControllerDB mappings.",
                    false,
                ),
                opt("keep", "Keep native APIs", "", false),
            ],
        ));
    }

    // 6. Build system -----------------------------------------------------------------------------
    if p.cfamily && p.engine.is_none() {
        let mut opts = Vec::new();
        if p.has_cmake {
            opts.push(opt(
                "keep",
                "Keep existing CMake",
                "Add a crossfire/crossfire.cmake include with per-OS settings.",
                true,
            ));
            opts.push(opt("cmake", "Regenerate CMake", "Replace CMakeLists.txt with a generated one (original kept as CMakeLists.original.txt).", false));
        } else {
            opts.push(opt("cmake", "Generate CMake", "CMakeLists.txt + CMakePresets.json with per-OS sections (Windows, macOS bundle, Linux, FreeBSD).", true));
            if p.has_msbuild || p.has_xcode || p.has_make {
                opts.push(opt(
                    "keep",
                    "Keep current build files",
                    "Only usable on the original OS.",
                    false,
                ));
            }
        }
        opts.push(opt(
            "meson",
            "Generate Meson",
            "meson.build with per-OS dependencies.",
            false,
        ));
        qs.push(single(
            "buildSystem",
            "Build system",
            "How the converted project is built on every OS.",
            opts,
        ));
        if scan.languages.iter().any(|l| l.name.contains("C++")) {
            qs.push(single(
                "cppStandard",
                "C++ standard",
                "std::filesystem needs C++17; C++20 adds <bit>, std::jthread, std::format.",
                vec![
                    opt("17", "C++17", "Widest compiler support.", false),
                    opt(
                        "20",
                        "C++20",
                        "Good support in current MSVC, Clang (Xcode 15+), GCC 11+.",
                        true,
                    ),
                    opt(
                        "23",
                        "C++23",
                        "Newest; check compiler versions on FreeBSD/older distros.",
                        false,
                    ),
                ],
            ));
        }
    } else if p.dotnet {
        qs.push(single(
            "buildSystem",
            "Build system",
            "SDK-style projects build everywhere with `dotnet publish -r <rid>`.",
            vec![
                opt(
                    "keep",
                    "Keep SDK-style project",
                    "Retarget TFMs and add RuntimeIdentifiers.",
                    true,
                ),
                opt(
                    "cake",
                    "Add build.cake script",
                    "Scripted multi-RID publish.",
                    false,
                ),
            ],
        ));
    }

    // 7. Rewrite style ---------------------------------------------------------------------------------
    qs.push(single(
        "style",
        "Rewrite style",
        "Strict keeps original API names and adds compatibility shims; idiomatic rewrites calls to portable APIs.",
        vec![
            opt("strict", "Strict (behavior-preserving)", "Smallest diffs: shims keep Sleep/GetTickCount/MessageBox working everywhere.", true),
            opt("idiomatic", "Idiomatic", "Rewrite to cf_* / standard portable calls; cleaner but larger diffs.", false),
        ],
    ));

    if p.cfamily {
        qs.push(toggle(
            "fixIncludeCase",
            "Fix #include casing",
            "Windows and macOS file systems are case-insensitive; Linux/FreeBSD are not. Correct include paths to match files on disk.",
            true,
        ));
    }

    // 8. macOS architecture / bundle id ------------------------------------------------------------------
    if to_mac && (p.cfamily || p.rust || p.go || p.dotnet) {
        qs.push(single(
            "macosArch",
            "macOS architecture",
            "Apple Silicon is arm64; Intel Macs are x86_64.",
            vec![
                opt(
                    "universal",
                    "Universal 2 (arm64 + x86_64)",
                    "One binary for all Macs.",
                    true,
                ),
                opt(
                    "arm64",
                    "Apple Silicon only",
                    "Smaller, simpler (x86 SIMD code must still be ported).",
                    false,
                ),
                opt(
                    "x86_64",
                    "Intel only",
                    "Runs on Apple Silicon via Rosetta 2.",
                    false,
                ),
            ],
        ));
    }
    let slug: String = scan
        .name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_lowercase();
    qs.push(Question {
        id: "bundleId".into(),
        title: "Application identifier".into(),
        description: "Reverse-DNS id used for the macOS bundle, Linux .desktop/AppStream and Windows installer.".into(),
        kind: QuestionKind::Text,
        options: vec![],
        default: json!(format!("com.example.{}", if slug.is_empty() { "app".into() } else { slug })),
        depends_on: None,
    });

    // 9. CI + packages ----------------------------------------------------------------------------------
    qs.push(toggle(
        "ci",
        "Generate CI workflow",
        "GitHub Actions building on windows-latest, macos-latest, ubuntu-latest and FreeBSD (vmactions/freebsd-vm).",
        true,
    ));
    let mut pkg_opts = Vec::new();
    let mut pkg_default = Vec::new();
    for t in &targets {
        match t {
            Os::Linux => {
                pkg_opts.push(opt(
                    "appimage",
                    "Linux AppImage",
                    "Single-file app via linuxdeploy.",
                    true,
                ));
                pkg_opts.push(opt("tarball", "Linux .tar.gz", "Plain archive.", false));
                pkg_default.push("appimage");
            }
            Os::Macos => {
                pkg_opts.push(opt(
                    "app",
                    "macOS .app bundle",
                    "Info.plist + bundle layout.",
                    true,
                ));
                pkg_opts.push(opt(
                    "dmg",
                    "macOS .dmg",
                    "Disk image containing the .app (hdiutil).",
                    true,
                ));
                pkg_default.extend(["app", "dmg"]);
            }
            Os::Windows => {
                pkg_opts.push(opt("zip", "Windows .zip", "Portable folder.", true));
                pkg_opts.push(opt(
                    "nsis",
                    "Windows installer (NSIS)",
                    "Setup .exe built with makensis.",
                    false,
                ));
                pkg_default.push("zip");
            }
            Os::Freebsd => {
                pkg_opts.push(opt(
                    "freebsd-pkg",
                    "FreeBSD pkg",
                    "Package built with `pkg create`.",
                    true,
                ));
                pkg_default.push("freebsd-pkg");
            }
        }
    }
    qs.push(Question {
        id: "packages".into(),
        title: "Package outputs".into(),
        description: "Packaging scripts to generate for each target.".into(),
        kind: QuestionKind::Multi,
        options: pkg_opts,
        default: json!(pkg_default),
        depends_on: None,
    });

    // 10. AI + originals -----------------------------------------------------------------------------------
    let hard = scan
        .findings
        .iter()
        .filter(|f| f.severity >= Severity::High)
        .map(|f| f.file.as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    qs.push(toggle(
        "useAi",
        "Use AI for hard files",
        &format!("Send the {hard} file(s) with high/blocker findings to the configured AI model for a full rewrite (requires an API key in Settings)."),
        hard > 0,
    ));
    qs.push(toggle(
        "keepOriginals",
        "Keep original files alongside",
        "Write the unmodified file next to each changed one as <name>.orig for easy comparison.",
        false,
    ));
    qs
}

/// Fill unanswered questions with their defaults so convert sees a complete set.
pub fn effective_answers(scan: &ScanReport, targets: &[Os], answers: &Answers) -> Answers {
    let mut out = answers.clone();
    for q in questions(scan, targets) {
        out.entry(q.id).or_insert(q.default);
    }
    out
}
