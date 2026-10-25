//! File classification: maps a path to a language id (used by the rule table)
//! and a display name (used in `LanguageStat`).

use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lang {
    /// Stable id used by rules: "c", "cpp", "objc", "python", ...
    pub id: &'static str,
    /// Display name for the language breakdown.
    pub name: &'static str,
    /// Counts toward source lines / portability (as opposed to config/data files).
    pub code: bool,
}

const fn l(id: &'static str, name: &'static str, code: bool) -> Option<Lang> {
    Some(Lang { id, name, code })
}

/// Classify a file by name/extension. Returns None for files we do not read.
pub fn classify(path: &Path) -> Option<Lang> {
    let file = path.file_name()?.to_string_lossy().to_string();
    let lower = file.to_ascii_lowercase();

    // Exact file names first.
    match lower.as_str() {
        "cmakelists.txt" => return l("cmake", "CMake", false),
        "makefile" | "gnumakefile" | "bsdmakefile" => return l("make", "Make", false),
        "meson.build" | "meson_options.txt" => return l("meson", "Meson", false),
        "sconstruct" | "sconscript" => return l("python", "Python", true),
        "dockerfile" => return l("docker", "Dockerfile", false),
        "podfile" => return l("ruby", "Ruby", false),
        "package.swift" => return l("swift", "Swift", true),
        "build.zig" => return l("zig", "Zig", true),
        "project.godot" => return l("godot", "Godot project", false),
        "requirements.txt" => return l("manifest", "Config", false),
        _ => {}
    }

    let ext = Path::new(&lower).extension()?.to_string_lossy().to_string();
    match ext.as_str() {
        "c" => l("c", "C", true),
        "h" => l("h", "C/C++ Header", true),
        "cpp" | "cc" | "cxx" | "c++" | "cp" => l("cpp", "C++", true),
        "hpp" | "hh" | "hxx" | "h++" | "inl" | "ipp" | "tpp" | "ixx" | "cppm" => l("cpp", "C++", true),
        "m" => l("objc", "Objective-C", true),
        "mm" => l("objcpp", "Objective-C++", true),
        "swift" => l("swift", "Swift", true),
        "cs" => l("csharp", "C#", true),
        "fs" | "fsx" => l("fsharp", "F#", true),
        "vb" => l("vbnet", "VB.NET", true),
        "xaml" | "axaml" => l("xaml", "XAML", true),
        "rs" => l("rust", "Rust", true),
        "go" => l("go", "Go", true),
        "java" => l("java", "Java", true),
        "kt" | "kts" => l("kotlin", "Kotlin", true),
        "scala" => l("scala", "Scala", true),
        "py" | "pyw" | "pyx" => l("python", "Python", true),
        "js" | "mjs" | "cjs" | "jsx" => l("js", "JavaScript", true),
        "ts" | "tsx" | "mts" | "cts" => l("js", "TypeScript", true),
        "lua" => l("lua", "Lua", true),
        "gd" => l("gdscript", "GDScript", true),
        "zig" => l("zig", "Zig", true),
        "d" => l("dlang", "D", true),
        "nim" => l("nim", "Nim", true),
        "dart" => l("dart", "Dart", true),
        "rb" => l("ruby", "Ruby", true),
        "hlsl" | "hlsli" | "fx" | "fxh" | "usf" | "ush" => l("hlsl", "HLSL", true),
        "glsl" | "vert" | "frag" | "geom" | "comp" | "tesc" | "tese" | "vsh" | "fsh" => {
            l("glsl", "GLSL", true)
        }
        "metal" => l("msl", "Metal Shading Language", true),
        "wgsl" => l("wgsl", "WGSL", true),
        "shader" | "cginc" | "compute" => l("hlsl", "ShaderLab/HLSL", true),
        "gdshader" => l("glsl", "Godot Shader", true),
        "asm" | "s" | "nasm" | "masm" => l("asm", "Assembly", true),
        "sh" | "bash" | "zsh" | "ksh" => l("shell", "Shell", true),
        "bat" | "cmd" => l("batch", "Batch", true),
        "ps1" | "psm1" | "psd1" => l("powershell", "PowerShell", true),
        "cmake" => l("cmake", "CMake", false),
        "mk" | "mak" => l("make", "Make", false),
        "sln" | "vcxproj" | "vcproj" | "csproj" | "fsproj" | "vbproj" | "props" | "targets" => {
            l("msbuild", "MSBuild", false)
        }
        "pbxproj" | "xcconfig" => l("xcode", "Xcode", false),
        "plist" | "entitlements" => l("plist", "Property list", false),
        "gradle" => l("gradle", "Gradle", false),
        "rc" => l("winrc", "Windows Resource", false),
        "def" => l("winrc", "Module definition", false),
        "manifest" => l("winrc", "Windows manifest", false),
        "json" | "toml" | "yaml" | "yml" | "ini" | "cfg" | "conf" | "xml" => l("manifest", "Config", false),
        "desktop" => l("manifest", "Desktop entry", false),
        "uproject" | "uplugin" => l("manifest", "Unreal descriptor", false),
        "pro" | "pri" => l("qmake", "qmake", false),
        _ => None,
    }
}

/// Expand a rule language token into concrete language ids.
pub fn expand_token(tok: &str) -> Vec<&'static str> {
    match tok {
        // Headers are matched by every C-family rule.
        "c" => vec!["c", "h"],
        "cpp" => vec!["cpp", "h"],
        "objc" => vec!["objc", "objcpp", "h"],
        "cfam" => vec!["c", "h", "cpp", "objc", "objcpp"],
        "dotnet" => vec!["csharp", "fsharp", "vbnet"],
        "shaders" => vec!["hlsl", "glsl", "msl", "wgsl"],
        "scripts" => vec!["shell", "batch", "powershell"],
        "buildfiles" => vec!["cmake", "make", "meson", "msbuild", "xcode", "gradle", "qmake"],
        other => vec![leak(other)],
    }
}

fn leak(s: &str) -> &'static str {
    // Rule tables are parsed once into a static, so leaking a handful of
    // short strings is fine and keeps `Lang::id` comparisons cheap.
    Box::leak(s.to_string().into_boxed_str())
}

/// Is this language id part of the C family (preprocessor-aware)?
pub fn is_c_family(id: &str) -> bool {
    matches!(id, "c" | "h" | "cpp" | "objc" | "objcpp")
}

/// Single-line comment prefixes for a language, used to skip comment lines.
pub fn comment_prefixes(id: &str) -> &'static [&'static str] {
    match id {
        "c" | "h" | "cpp" | "objc" | "objcpp" | "swift" | "csharp" | "fsharp" | "rust" | "go"
        | "java" | "kotlin" | "scala" | "js" | "zig" | "dlang" | "dart" | "hlsl" | "glsl"
        | "msl" | "wgsl" => &["//", "/*", "* ", "*/"],
        "python" | "shell" | "powershell" | "cmake" | "make" | "meson" | "ruby" | "gdscript"
        | "nim" | "qmake" | "docker" => &["#"],
        "lua" => &["--"],
        "batch" => &["rem ", "REM ", "::", "@rem", "@REM"],
        "vbnet" => &["'"],
        "asm" => &[";"],
        _ => &[],
    }
}
