import { createContext, useContext, useEffect, useMemo, useState, type ReactNode } from 'react';
import { getEngine, type Engine } from './bridge';
import type { AiSettings, Answers, BuildResult, ConvertResult, Os, ProgressEvent, Question, ScanReport, Toolchain } from './types';

export type Step = 'source' | 'analyze' | 'configure' | 'targets' | 'convert' | 'build' | 'report';
export const STEPS: { id: Step; label: string }[] = [
  { id: 'source', label: 'Source' },
  { id: 'analyze', label: 'Analyze' },
  { id: 'targets', label: 'Targets' },
  { id: 'configure', label: 'Configure' },
  { id: 'convert', label: 'Convert' },
  { id: 'build', label: 'Build' },
  { id: 'report', label: 'Report' },
];

export interface LogLine extends ProgressEvent { t: number; }

interface Store {
  engine: Engine | null;
  hostOs: Os;
  step: Step;
  go: (s: Step) => void;
  reached: Set<Step>;

  source: string; setSource: (p: string) => void;
  sourceOs: Os[]; setSourceOs: (o: Os[]) => void;
  scan: ScanReport | null; setScan: (s: ScanReport | null) => void;
  targets: Os[]; setTargets: (t: Os[]) => void;
  questions: Question[]; setQuestions: (q: Question[]) => void;
  answers: Answers; setAnswer: (id: string, v: Answers[string]) => void;
  output: string; setOutput: (p: string) => void;
  buildAfter: boolean; setBuildAfter: (b: boolean) => void;
  convertResult: ConvertResult | null; setConvertResult: (r: ConvertResult | null) => void;
  buildResult: BuildResult | null; setBuildResult: (r: BuildResult | null) => void;
  toolchains: Toolchain[]; refreshToolchains: () => Promise<void>;

  ai: AiSettings; setAi: (a: AiSettings) => void;
  theme: 'dark' | 'light'; setTheme: (t: 'dark' | 'light') => void;
  settingsOpen: boolean; setSettingsOpen: (b: boolean) => void;

  log: LogLine[]; clearLog: () => void;
  reset: () => void;
}

const Ctx = createContext<Store | null>(null);

const load = <T,>(k: string, fallback: T): T => {
  try { const v = localStorage.getItem(k); return v ? (JSON.parse(v) as T) : fallback; } catch { return fallback; }
};
const save = (k: string, v: unknown) => { try { localStorage.setItem(k, JSON.stringify(v)); } catch { /* storage unavailable */ } };

const DEFAULT_AI: AiSettings = { enabled: true, provider: 'anthropic', model: 'claude-opus-5-5', apiKey: '' };

export function StoreProvider({ children }: { children: ReactNode }) {
  const [engine, setEngine] = useState<Engine | null>(null);
  const [hostOs, setHostOs] = useState<Os>('macos');
  const [step, setStep] = useState<Step>('source');
  const [reached, setReached] = useState<Set<Step>>(new Set(['source']));
  const [source, setSource] = useState('');
  const [sourceOs, setSourceOs] = useState<Os[]>([]);
  const [scan, setScan] = useState<ScanReport | null>(null);
  const [targets, setTargets] = useState<Os[]>([]);
  const [questions, setQuestions] = useState<Question[]>([]);
  const [answers, setAnswers] = useState<Answers>({});
  const [output, setOutput] = useState('');
  const [buildAfter, setBuildAfter] = useState(true);
  const [convertResult, setConvertResult] = useState<ConvertResult | null>(null);
  const [buildResult, setBuildResult] = useState<BuildResult | null>(null);
  const [toolchains, setToolchains] = useState<Toolchain[]>([]);
  const [ai, setAiState] = useState<AiSettings>(() => ({ ...DEFAULT_AI, ...load<Partial<AiSettings>>('cf.ai', {}) }));
  const [theme, setThemeState] = useState<'dark' | 'light'>(() => load('cf.theme', 'dark'));
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [log, setLog] = useState<LogLine[]>([]);

  useEffect(() => {
    getEngine().then(async (e) => {
      setEngine(e);
      setHostOs(await e.hostOs());
      setToolchains(await e.toolchains());
    });
  }, []);

  useEffect(() => {
    if (!engine) return;
    return engine.onProgress((e) => setLog((l) => [...l.slice(-800), { ...e, t: Date.now() }]));
  }, [engine]);

  useEffect(() => { document.documentElement.dataset.theme = theme; }, [theme]);

  const value = useMemo<Store>(() => ({
    engine, hostOs, step, reached,
    go: (s) => { setStep(s); setReached((r) => new Set(r).add(s)); },
    source, setSource, sourceOs, setSourceOs, scan, setScan, targets, setTargets,
    questions, setQuestions, answers, setAnswer: (id, v) => setAnswers((a) => ({ ...a, [id]: v })),
    output, setOutput, buildAfter, setBuildAfter, convertResult, setConvertResult, buildResult, setBuildResult,
    toolchains, refreshToolchains: async () => { if (engine) setToolchains(await engine.toolchains()); },
    ai, setAi: (a) => { setAiState(a); save('cf.ai', a); },
    theme, setTheme: (t) => { setThemeState(t); save('cf.theme', t); },
    settingsOpen, setSettingsOpen,
    log, clearLog: () => setLog([]),
    reset: () => {
      setStep('source'); setReached(new Set(['source'])); setSource(''); setSourceOs([]); setScan(null);
      setTargets([]); setQuestions([]); setAnswers({}); setOutput(''); setConvertResult(null); setBuildResult(null); setLog([]);
    },
  }), [engine, hostOs, step, reached, source, sourceOs, scan, targets, questions, answers, output, buildAfter,
    convertResult, buildResult, toolchains, ai, theme, settingsOpen, log]);

  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useStore() {
  const s = useContext(Ctx);
  if (!s) throw new Error('useStore outside StoreProvider');
  return s;
}
