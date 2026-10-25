// Shared contract between the Crossfire UI and the Rust engine.
// Mirrors engine/src/model.rs — every struct is serialized with camelCase keys.

export type Os = 'windows' | 'macos' | 'linux' | 'freebsd';

export type Category =
  | 'graphics' | 'windowing' | 'audio' | 'input' | 'filesystem' | 'threading'
  | 'networking' | 'process' | 'registry' | 'timing' | 'dynamicLibrary'
  | 'ui' | 'build' | 'packaging' | 'other';

export type Severity = 'info' | 'low' | 'medium' | 'high' | 'blocker';

export interface LanguageStat { name: string; files: number; lines: number; }

export interface ProjectKind {
  /** e.g. "CMake", "MSBuild", "Xcode", "Cargo", "Unity", "Godot", "Unreal", "Electron", ".NET", "Gradle" */
  name: string;
  /** "game" | "app" | "library" | "cli" | "service" | "unknown" */
  flavor: string;
  evidence: string;
}

export interface Finding {
  id: string;
  file: string;
  line: number;
  snippet: string;
  category: Category;
  severity: Severity;
  /** OS that this API ties the code to */
  os: Os[];
  api: string;
  message: string;
  /** suggestion per target OS */
  suggestions: Partial<Record<Os, string>>;
}

export interface ScanReport {
  root: string;
  name: string;
  detectedOs: Os[];
  languages: LanguageStat[];
  kinds: ProjectKind[];
  totalFiles: number;
  totalLines: number;
  /** 0..100 — share of lines with no OS-specific findings */
  portability: number;
  findings: Finding[];
  categories: Partial<Record<Category, number>>;
  dependencies: { name: string; source: string; portable: boolean | null; note: string }[];
  durationMs: number;
}

export interface QuestionOption { value: string; label: string; description: string; recommended?: boolean; }

export interface Question {
  id: string;
  title: string;
  description: string;
  kind: 'single' | 'multi' | 'text' | 'toggle';
  options: QuestionOption[];
  default: string | string[] | boolean;
  /** only shown when some answer matches */
  dependsOn?: { question: string; value: string };
}

export type Answers = Record<string, string | string[] | boolean>;

export interface AiSettings {
  enabled: boolean;
  provider: 'anthropic' | 'openaiCompatible' | 'none';
  model: string;
  apiKey: string;
  baseUrl?: string;
}

export interface ConvertRequest {
  source: string;
  output: string;
  sourceOs: Os[];
  targets: Os[];
  answers: Answers;
  ai: AiSettings;
}

export interface FileChange {
  path: string;
  action: 'added' | 'modified' | 'copied' | 'removed';
  method: 'rule' | 'ai' | 'template' | 'copy';
  confidence: number; // 0..1
  notes: string[];
  diff?: string;
}

export interface ConvertResult {
  output: string;
  changes: FileChange[];
  manualReview: Finding[];
  reportPath: string;
  durationMs: number;
}

export interface Toolchain {
  id: string;
  name: string;
  found: boolean;
  version?: string;
  path?: string;
  /** targets this toolchain can produce from the current host */
  targets: Os[];
  hint: string;
}

export interface BuildRequest { project: string; targets: Os[]; release: boolean; package: boolean; }

export interface BuildTargetResult {
  target: Os;
  status: 'success' | 'failed' | 'skipped' | 'ci';
  artifacts: string[];
  log: string;
  reason?: string;
}

export interface BuildResult { results: BuildTargetResult[]; ciWorkflow?: string; }

export interface ProgressEvent {
  stage: 'scan' | 'convert' | 'build' | 'test';
  message: string;
  /** 0..1, or -1 if unknown */
  progress: number;
  file?: string;
  level: 'info' | 'warn' | 'error' | 'success';
}
