import { AnimatePresence, motion } from 'framer-motion';
import { AlertTriangle, ArrowRight, ChevronDown, Package, RotateCcw, Search } from 'lucide-react';
import { useEffect, useMemo, useRef, useState } from 'react';
import { useStore } from '../state';
import type { Category, Finding, Os, Severity } from '../types';
import { ActionBar, Gauge, OS_META, OsGlyph, PageHeader, Reveal, Sev } from '../components/ui';

const SEV_ORDER: Severity[] = ['blocker', 'high', 'medium', 'low', 'info'];

export function AnalyzeView() {
  const { engine, source, scan, setScan, sourceOs, setSourceOs, log, go } = useStore();
  const [error, setError] = useState<string | null>(null);
  const started = useRef(false);

  const run = async () => {
    if (!engine) return;
    setError(null);
    setScan(null);
    try {
      const r = await engine.scan(source);
      setScan(r);
      if (sourceOs.length === 0) setSourceOs(r.detectedOs);
    } catch (e) {
      setError(String(e));
    }
  };

  useEffect(() => {
    if (!scan && engine && !started.current) { started.current = true; run(); }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [engine]);

  const last = [...log].reverse().find((l) => l.stage === 'scan');

  if (error) {
    return (
      <div className="page-inner">
        <PageHeader step={2} eyebrow="Analyze" title="The scan hit a problem" />
        <div className="glass card"><div className="code">{error}</div>
          <button className="btn" style={{ marginTop: 16 }} onClick={run}><RotateCcw size={16} /> Try again</button></div>
      </div>
    );
  }

  if (!scan) {
    return (
      <div className="page-inner">
        <div className="scan-stage">
          <div>
            <div className="radar" style={{ margin: '0 auto 30px' }}>
              <div className="ring" /><div className="ring" /><div className="ring" />
              <div className="sweep" /><div className="core" />
              {[[30, 60], [170, 50], [60, 170], [190, 160], [120, 26]].map(([x, y], i) => (
                <motion.span key={i} className="blip" style={{ left: x, top: y }}
                  animate={{ opacity: [0, 1, 0] }} transition={{ duration: 2.2, repeat: Infinity, delay: i * .42 }} />
              ))}
            </div>
            <div className="eyebrow" style={{ justifyContent: 'center' }}><b>Step 2</b> · Analyze</div>
            <h1 className="title" style={{ fontSize: 32 }}>Mapping your <em>platform DNA</em></h1>
            <div className="progress-xl" style={{ width: 420, margin: '18px auto 12px' }}>
              <i style={{ width: `${Math.max(4, (last?.progress ?? 0) * 100)}%` }} />
            </div>
            <div className="scan-file" style={{ margin: '0 auto' }}>{last?.file ?? 'Indexing files…'}</div>
          </div>
        </div>
      </div>
    );
  }

  return <ScanResults onRescan={run} onNext={() => go('targets')} />;
}

function ScanResults({ onRescan, onNext }: { onRescan: () => void; onNext: () => void }) {
  const { scan } = useStore();
  const s = scan!;
  const [q, setQ] = useState('');
  const [sev, setSev] = useState<Severity | 'all'>('all');
  const [cat, setCat] = useState<Category | 'all'>('all');
  const [open, setOpen] = useState<string | null>(null);

  const maxLines = Math.max(...s.languages.map((l) => l.lines), 1);
  const cats = Object.entries(s.categories).sort((a, b) => (b[1] ?? 0) - (a[1] ?? 0)) as [Category, number][];
  const maxCat = Math.max(...cats.map((c) => c[1]), 1);
  const bySev = SEV_ORDER.map((x) => [x, s.findings.filter((f) => f.severity === x).length] as const);

  const shown = useMemo(() => s.findings
    .filter((f) => sev === 'all' || f.severity === sev)
    .filter((f) => cat === 'all' || f.category === cat)
    .filter((f) => !q || `${f.api} ${f.file} ${f.message}`.toLowerCase().includes(q.toLowerCase()))
    .sort((a, b) => SEV_ORDER.indexOf(a.severity) - SEV_ORDER.indexOf(b.severity)), [s, q, sev, cat]);

  return (
    <div className="page-inner">
      <PageHeader step={2} eyebrow="Analyze" title={<><em>{s.name}</em> is {Math.round(s.portability)}% portable</>}
        lede={`Crossfire read ${s.totalFiles.toLocaleString()} files (${s.totalLines.toLocaleString()} lines) in ${(s.durationMs / 1000).toFixed(1)}s and found ${s.findings.length} places tied to a specific OS.`} />

      <div className="grid" style={{ gridTemplateColumns: '1.25fr 1fr', marginBottom: 16 }}>
        <Reveal className="glass gauge-wrap">
          <Gauge value={s.portability} label="portable" />
          <div className="gauge-text">
            <div className="big">Written for</div>
            <div className="row" style={{ gap: 8, margin: '8px 0 16px', flexWrap: 'wrap' }}>
              {s.detectedOs.map((o: Os) => (
                <span key={o} className={`chip on ${OS_META[o].cls}`}><span style={{ color: 'var(--os)', display: 'flex' }}><OsGlyph os={o} size={14} /></span>{OS_META[o].name}</span>
              ))}
            </div>
            <div className="row" style={{ gap: 6, flexWrap: 'wrap' }}>
              {bySev.filter(([, n]) => n > 0).map(([x, n]) => <span key={x} className={`badge sev-${x}`}>{n} {x}</span>)}
            </div>
          </div>
        </Reveal>
        <Reveal delay={.05} className="glass card">
          <h2 className="section">Languages</h2>
          {s.languages.slice(0, 6).map((l) => (
            <div key={l.name} className="lang-row">
              <span style={{ fontWeight: 560 }}>{l.name}</span>
              <div className="bar"><motion.i initial={{ width: 0 }} animate={{ width: `${(l.lines / maxLines) * 100}%` }} transition={{ duration: 1 }} /></div>
              <span className="muted small" style={{ textAlign: 'right' }}>{l.lines.toLocaleString()}</span>
            </div>
          ))}
        </Reveal>
      </div>

      <div className="grid g4" style={{ marginBottom: 16 }}>
        {s.kinds.slice(0, 4).map((k, i) => (
          <Reveal key={k.name} delay={.08 + i * .04} className="glass stat">
            <div className="k">{k.flavor}</div>
            <div className="v" style={{ fontSize: 20 }}>{k.name}</div>
            <div className="muted small" style={{ marginTop: 4 }}>{k.evidence}</div>
          </Reveal>
        ))}
      </div>

      <Reveal delay={.12} className="glass card" style={{ marginBottom: 16 }}>
        <h2 className="section">Where your code touches the OS</h2>
        <div className="heat">
          {cats.map(([c, n]) => (
            <button key={c} className="heat-cell" onClick={() => setCat(cat === c ? 'all' : c)}
              style={{ background: `rgba(255, 46, 99, ${0.04 + (n / maxCat) * 0.22})`, cursor: 'pointer', textAlign: 'left',
                outline: cat === c ? '1.5px solid var(--fire-2)' : 'none', color: 'inherit' }}>
              <div className="n">{n}</div><div className="l">{c.replace(/([A-Z])/g, ' $1')}</div>
            </button>
          ))}
        </div>
      </Reveal>

      <Reveal delay={.16} className="glass" style={{ marginBottom: 16, overflow: 'hidden' }}>
        <div className="toolbar">
          <h2 className="section" style={{ margin: 0 }}>Findings <span className="count">{shown.length} of {s.findings.length}</span></h2>
          <span className="spacer" />
          <div className="search"><Search size={15} /><input placeholder="Search APIs, files…" value={q} onChange={(e) => setQ(e.target.value)} /></div>
          {(['all', ...SEV_ORDER] as const).map((x) => (
            <button key={x} className={`chip ${sev === x ? 'on' : ''}`} onClick={() => setSev(x)}>{x}</button>
          ))}
        </div>
        <div className="findings">
          {shown.map((f) => <FindingRow key={f.id} f={f} open={open === f.id} onToggle={() => setOpen(open === f.id ? null : f.id)} />)}
          {shown.length === 0 && <div className="empty">Nothing matches these filters.</div>}
        </div>
      </Reveal>

      {s.dependencies.length > 0 && (
        <Reveal delay={.2} className="glass card">
          <h2 className="section"><Package size={16} /> Dependencies</h2>
          <div className="grid g2">
            {s.dependencies.map((d) => (
              <div key={d.name + d.source} className="row" style={{ padding: '10px 12px', borderRadius: 12, background: 'var(--panel)' }}>
                <span className="dot" style={d.portable === false ? { background: 'var(--bad)', boxShadow: '0 0 0 4px rgba(248,113,113,.15)' } :
                  d.portable == null ? { background: 'var(--text-3)', boxShadow: 'none' } : undefined} />
                <div style={{ minWidth: 0 }}>
                  <div style={{ fontWeight: 600 }}>{d.name} <span className="muted small">· {d.source}</span></div>
                  <div className="muted small">{d.note}</div>
                </div>
              </div>
            ))}
          </div>
        </Reveal>
      )}

      <ActionBar>
        <AlertTriangle size={18} color="var(--warn)" />
        <span className="hint">{s.findings.filter((f) => f.severity === 'blocker').length} blockers will be handled by the conversion strategy you pick next.</span>
        <span className="spacer" />
        <button className="btn ghost" onClick={onRescan}><RotateCcw size={16} /> Rescan</button>
        <button className="btn primary lg" onClick={onNext}>Choose targets <ArrowRight size={18} /></button>
      </ActionBar>
    </div>
  );
}

function FindingRow({ f, open, onToggle }: { f: Finding; open: boolean; onToggle: () => void }) {
  return (
    <div>
      <div className="finding" onClick={onToggle}>
        <Sev s={f.severity} />
        <div style={{ minWidth: 0 }}>
          <div className="api">{f.api}</div>
          <div className="small" style={{ color: 'var(--text-2)' }}>{f.message}</div>
          <div className="loc">{f.file}:{f.line}</div>
        </div>
        <motion.span animate={{ rotate: open ? 180 : 0 }} style={{ color: 'var(--text-3)' }}><ChevronDown size={18} /></motion.span>
      </div>
      <AnimatePresence initial={false}>
        {open && (
          <motion.div initial={{ height: 0, opacity: 0 }} animate={{ height: 'auto', opacity: 1 }} exit={{ height: 0, opacity: 0 }} style={{ overflow: 'hidden' }}>
            <div style={{ padding: '0 18px 18px 118px' }}>
              <div className="code">{f.snippet}</div>
              <div style={{ marginTop: 12 }}>
                {(Object.entries(f.suggestions) as [Os, string][]).map(([o, sug]) => (
                  <div key={o} className={`sugg ${OS_META[o].cls}`}>
                    <span style={{ color: 'var(--os)', display: 'flex', paddingTop: 2 }}><OsGlyph os={o} size={16} /></span>
                    <span><b style={{ color: 'var(--text)' }}>{OS_META[o].name}:</b> {sug}</span>
                  </div>
                ))}
              </div>
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
