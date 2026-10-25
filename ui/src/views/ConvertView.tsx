import { ArrowRight, FileCode2, FilePlus2, FolderOpen, Files, RotateCcw } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { useStore } from '../state';
import type { FileChange } from '../types';
import { Console, Diff } from '../components/Console';
import { ActionBar, PageHeader, Pipeline, Reveal } from '../components/ui';

const PHASES = ['Copy', 'Rewrite', 'AI pass', 'Generate', 'Report'];

function phaseOf(msg: string): number {
  const m = msg.toLowerCase();
  if (m.includes('report') || m.startsWith('done')) return 4;
  if (m.includes('generat') || m.includes('cmake') || m.includes('workflow')) return 3;
  if (m.startsWith('ai') || m.includes(' ai ')) return 2;
  if (m.includes('rewrit') || m.includes('shim') || m.includes('replac')) return 1;
  return 0;
}

export function ConvertView() {
  const { engine, source, output, sourceOs, targets, answers, ai, convertResult, setConvertResult, setBuildResult, log, buildAfter, go } = useStore();
  const [since, setSince] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [sel, setSel] = useState<FileChange | null>(null);
  const started = useRef(false);

  const run = async () => {
    if (!engine) return;
    setError(null); setConvertResult(null); setBuildResult(null); setSince(Date.now());
    try {
      const r = await engine.convert({
        source, output, sourceOs, targets, answers,
        ai: { ...ai, enabled: ai.enabled && answers.useAi === true && !!ai.apiKey },
      });
      setConvertResult(r);
      setSel(r.changes.find((c) => c.diff) ?? null);
    } catch (e) { setError(String(e)); }
  };

  useEffect(() => {
    if (!convertResult && engine && !started.current) { started.current = true; run(); }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [engine]);

  const convLog = log.filter((l) => l.stage === 'convert' && l.t >= since);
  const last = convLog[convLog.length - 1];
  const phase = convertResult ? PHASES.length : last ? phaseOf(last.message) : 0;

  // The engine lists every untouched file as "copied"; show those as one summary row
  const edited = convertResult?.changes.filter((c) => c.action !== 'copied' || c.method !== 'copy') ?? [];
  const copied = (convertResult?.changes.length ?? 0) - edited.length;

  const counts = convertResult ? {
    added: convertResult.changes.filter((c) => c.action === 'added').length,
    modified: convertResult.changes.filter((c) => c.action === 'modified').length,
    ai: convertResult.changes.filter((c) => c.method === 'ai').length,
    review: convertResult.manualReview.length,
  } : null;

  return (
    <div className="page-inner">
      <PageHeader step={5} eyebrow="Convert"
        title={convertResult ? <>Your port is <em>ready</em></> : error ? 'Conversion stopped' : <>Forging <em>{targets.length} platforms</em></>}
        lede={convertResult ? `Written to ${convertResult.output} in ${(convertResult.durationMs / 1000).toFixed(1)}s.` : 'Rewriting OS-specific code, generating build files and packaging scripts.'} />

      <Reveal style={{ marginBottom: 16 }}><Pipeline steps={PHASES} current={phase} /></Reveal>

      {!convertResult && (
        <Reveal delay={.05} style={{ marginBottom: 16 }}>
          <div className="progress-xl"><i style={{ width: `${Math.max(3, (last?.progress ?? 0) * 100)}%` }} /></div>
        </Reveal>
      )}

      {error && <div className="glass card" style={{ marginBottom: 16 }}><div className="code">{error}</div></div>}

      {counts && (
        <div className="grid g4" style={{ marginBottom: 16 }}>
          {[['Files added', counts.added, FilePlus2], ['Files rewritten', counts.modified, FileCode2],
            ['Rewritten by AI', counts.ai, Files], ['Need your review', counts.review, RotateCcw]].map(([k, v, Icon], i) => {
            const I = Icon as typeof Files;
            return (
              <Reveal key={k as string} delay={i * .05} className="glass stat">
                <div className="k row" style={{ gap: 6 }}><I size={14} />{k as string}</div>
                <div className="v">{v as number}</div>
              </Reveal>
            );
          })}
        </div>
      )}

      {convertResult ? (
        <div className="grid" style={{ gridTemplateColumns: 'minmax(0, 1fr) minmax(0, 1.35fr)', alignItems: 'start' }}>
          <Reveal className="glass" style={{ overflow: 'hidden' }}>
            <div className="toolbar"><h2 className="section" style={{ margin: 0 }}>Changes <span className="count">{edited.length}</span></h2></div>
            {edited.map((c) => (
              <div key={c.path} className="change" onClick={() => setSel(c)} style={sel?.path === c.path ? { background: 'var(--fire-soft)' } : undefined}>
                <span style={{ color: c.action === 'added' ? 'var(--ok)' : c.action === 'modified' ? 'var(--warn)' : 'var(--text-3)', fontWeight: 800 }}>
                  {c.action === 'added' ? 'A' : c.action === 'modified' ? 'M' : c.action === 'removed' ? 'D' : 'C'}
                </span>
                <span className="mono">{c.path}</span>
                <span className={`method ${c.method}`}>{c.method}</span>
                <span className="small muted">{Math.round(c.confidence * 100)}%</span>
              </div>
            ))}
            {copied > 0 && (
              <div className="change" style={{ cursor: 'default' }}>
                <span style={{ color: 'var(--text-3)', fontWeight: 800 }}>C</span>
                <span className="muted">{copied.toLocaleString()} files copied unchanged</span>
                <span className="method copy">copy</span><span />
              </div>
            )}
          </Reveal>
          <Reveal delay={.05} className="glass" style={{ overflow: 'hidden', position: 'sticky', top: 0 }}>
            <div className="toolbar">
              <h2 className="section" style={{ margin: 0 }} title={sel?.path}>{sel ? sel.path : 'Select a file'}</h2>
            </div>
            {sel && (
              <div style={{ padding: '0 18px 12px' }}>
                {sel.notes.map((n) => <div key={n} className="small" style={{ color: 'var(--text-2)' }}>• {n}</div>)}
              </div>
            )}
            {sel?.diff ? <Diff text={sel.diff} /> : <div className="empty">{sel ? 'New or copied file — no diff to show.' : 'Pick a change on the left to see the diff.'}</div>}
          </Reveal>
        </div>
      ) : (
        <Console lines={log} since={since} title="crossfire convert" />
      )}

      <ActionBar>
        {convertResult ? (
          <>
            <button className="btn" onClick={() => engine?.openPath(convertResult.output)}><FolderOpen size={16} /> Open folder</button>
            <button className="btn ghost" onClick={() => { started.current = true; run(); }}><RotateCcw size={16} /> Convert again</button>
            <span className="spacer" />
            {buildAfter
              ? <button className="btn primary lg" onClick={() => go('build')}>Build all targets <ArrowRight size={18} /></button>
              : <button className="btn primary lg" onClick={() => go('report')}>View report <ArrowRight size={18} /></button>}
          </>
        ) : (
          <>
            <span className="hint">{last?.message ?? 'Starting…'}</span><span className="spacer" />
            {error && <button className="btn primary" onClick={run}><RotateCcw size={16} /> Retry</button>}
          </>
        )}
      </ActionBar>
    </div>
  );
}
