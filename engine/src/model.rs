//! Data model shared with the UI. Mirrors `ui/src/types.ts` exactly: every
//! struct uses camelCase keys and every enum serializes as a camelCase string.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Os {
    Windows,
    Macos,
    Linux,
    Freebsd,
}

impl Os {
    pub const ALL: [Os; 4] = [Os::Windows, Os::Macos, Os::Linux, Os::Freebsd];

    /// The OS this engine binary is running on (FreeBSD/other unixes map sensibly).
    pub fn host() -> Os {
        match std::env::consts::OS {
            "windows" => Os::Windows,
            "macos" => Os::Macos,
            "freebsd" => Os::Freebsd,
            _ => Os::Linux,
        }
    }

    /// Lower-case id as used on the wire ("windows", "macos", ...).
    pub fn id(self) -> &'static str {
        match self {
            Os::Windows => "windows",
            Os::Macos => "macos",
            Os::Linux => "linux",
            Os::Freebsd => "freebsd",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Os::Windows => "Windows",
            Os::Macos => "macOS",
            Os::Linux => "Linux",
            Os::Freebsd => "FreeBSD",
        }
    }

    /// C preprocessor test that is true when compiling for this OS.
    pub fn c_macro(self) -> &'static str {
        match self {
            Os::Windows => "defined(_WIN32)",
            Os::Macos => "defined(__APPLE__)",
            Os::Linux => "defined(__linux__)",
            Os::Freebsd => "defined(__FreeBSD__)",
        }
    }

    pub fn parse(s: &str) -> Option<Os> {
        match s.trim().to_ascii_lowercase().as_str() {
            "windows" | "win" | "win32" | "win64" => Some(Os::Windows),
            "macos" | "mac" | "osx" | "darwin" => Some(Os::Macos),
            "linux" => Some(Os::Linux),
            "freebsd" | "bsd" => Some(Os::Freebsd),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Category {
    Graphics,
    Windowing,
    Audio,
    Input,
    Filesystem,
    Threading,
    Networking,
    Process,
    Registry,
    Timing,
    DynamicLibrary,
    Ui,
    Build,
    Packaging,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    Info,
    Low,
    Medium,
    High,
    Blocker,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageStat {
    pub name: String,
    pub files: usize,
    pub lines: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectKind {
    pub name: String,
    /// "game" | "app" | "library" | "cli" | "service" | "unknown"
    pub flavor: String,
    pub evidence: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub id: String,
    pub file: String,
    pub line: usize,
    pub snippet: String,
    pub category: Category,
    pub severity: Severity,
    pub os: Vec<Os>,
    pub api: String,
    pub message: String,
    pub suggestions: BTreeMap<Os, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Dependency {
    pub name: String,
    pub source: String,
    /// Serialized as `null` when unknown (the TS type is `boolean | null`).
    pub portable: Option<bool>,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanReport {
    pub root: String,
    pub name: String,
    pub detected_os: Vec<Os>,
    pub languages: Vec<LanguageStat>,
    pub kinds: Vec<ProjectKind>,
    pub total_files: usize,
    pub total_lines: usize,
    pub portability: f64,
    pub findings: Vec<Finding>,
    pub categories: BTreeMap<Category, usize>,
    pub dependencies: Vec<Dependency>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionOption {
    pub value: String,
    pub label: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recommended: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum QuestionKind {
    Single,
    Multi,
    Text,
    Toggle,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DependsOn {
    pub question: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Question {
    pub id: String,
    pub title: String,
    pub description: String,
    pub kind: QuestionKind,
    pub options: Vec<QuestionOption>,
    /// string | string[] | boolean
    pub default: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depends_on: Option<DependsOn>,
}

/// `Record<string, string | string[] | boolean>`
pub type Answers = HashMap<String, serde_json::Value>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AiProvider {
    Anthropic,
    OpenaiCompatible,
    #[default]
    None,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub provider: AiProvider,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertRequest {
    pub source: String,
    pub output: String,
    #[serde(default)]
    pub source_os: Vec<Os>,
    pub targets: Vec<Os>,
    #[serde(default)]
    pub answers: Answers,
    #[serde(default)]
    pub ai: AiSettings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FileAction {
    Added,
    Modified,
    Copied,
    Removed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChangeMethod {
    Rule,
    Ai,
    Template,
    Copy,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChange {
    pub path: String,
    pub action: FileAction,
    pub method: ChangeMethod,
    pub confidence: f64,
    pub notes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diff: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertResult {
    pub output: String,
    pub changes: Vec<FileChange>,
    pub manual_review: Vec<Finding>,
    pub report_path: String,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Toolchain {
    pub id: String,
    pub name: String,
    pub found: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub targets: Vec<Os>,
    pub hint: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildRequest {
    pub project: String,
    pub targets: Vec<Os>,
    #[serde(default)]
    pub release: bool,
    #[serde(default)]
    pub package: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BuildStatus {
    Success,
    Failed,
    Skipped,
    Ci,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildTargetResult {
    pub target: Os,
    pub status: BuildStatus,
    pub artifacts: Vec<String>,
    pub log: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildResult {
    pub results: Vec<BuildTargetResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ci_workflow: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Stage {
    Scan,
    Convert,
    Build,
    Test,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Level {
    Info,
    Warn,
    Error,
    Success,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressEvent {
    pub stage: Stage,
    pub message: String,
    /// 0..1, or -1 if unknown
    pub progress: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    pub level: Level,
}

impl ProgressEvent {
    pub fn new(stage: Stage, level: Level, progress: f64, message: impl Into<String>) -> Self {
        ProgressEvent {
            stage,
            message: message.into(),
            progress,
            file: None,
            level,
        }
    }

    pub fn with_file(mut self, file: impl Into<String>) -> Self {
        self.file = Some(file.into());
        self
    }
}

/// Typed accessors over the loosely-typed answers map.
pub trait AnswersExt {
    fn str_or(&self, key: &str, default: &str) -> String;
    fn bool_or(&self, key: &str, default: bool) -> bool;
    fn list(&self, key: &str) -> Vec<String>;
}

impl AnswersExt for Answers {
    fn str_or(&self, key: &str, default: &str) -> String {
        match self.get(key) {
            Some(serde_json::Value::String(s)) if !s.is_empty() => s.clone(),
            Some(serde_json::Value::Array(a)) => a
                .first()
                .and_then(|v| v.as_str())
                .unwrap_or(default)
                .to_string(),
            _ => default.to_string(),
        }
    }

    fn bool_or(&self, key: &str, default: bool) -> bool {
        match self.get(key) {
            Some(serde_json::Value::Bool(b)) => *b,
            Some(serde_json::Value::String(s)) => matches!(s.as_str(), "true" | "yes" | "1"),
            _ => default,
        }
    }

    fn list(&self, key: &str) -> Vec<String> {
        match self.get(key) {
            Some(serde_json::Value::Array(a)) => a
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect(),
            Some(serde_json::Value::String(s)) if !s.is_empty() => {
                s.split(',').map(|x| x.trim().to_string()).collect()
            }
            _ => Vec::new(),
        }
    }
}
