import { motion } from 'framer-motion';
import { ArrowRight, Check, Cloud, FolderOutput, Hammer, Laptop } from 'lucide-react';
import { useEffect, useState } from 'react';
import { ALL_OS } from '../mock';
import { useStore } from '../state';
import type { Os } from '../types';
import { ActionBar, OS_META, OsGlyph, PageHeader, Reveal, Switch } from '../components/ui';

export function TargetsView() {
  const { engine, scan, sourceOs, targets, setTargets, toolchains, source, output, setOutput, buildAfter, setBuildAfter, setQuestions, go, hostOs } = useStore();
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!output && source) {
      const base = source.replace(/[\\/]+$/, '');
      setOutput(`${base}-crossfire`);
    }
  }, [output, source, setOutput]);

  // Pre-select every OS that isn't the source
  useEffect(() => {
    if (targets.length === 0) setTargets(ALL_OS.filter((o) => !sourceOs.includes(o)));
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const toggle = (o: Os) => setTargets(targets.includes(o) ? targets.filter((x) => x !== o) : [...targets, o]);
  const localBuilders = (o: Os) => toolchains.filter((t) => t.found && t.targets.includes(o));

  const next = async () => {
    if (!engine || !scan) return;
    setLoading(true);
    try { setQuestions(await engine.questions(scan, targets)); go('configure'); } finally { setLoading(false); }
  };

  return (
    <div className="page-inner">
      <PageHeader step={3} eyebrow="Targets" title={<>Where should it <em>run next?</em></>}
        lede="Pick every platform you want a version for. Crossfire generates one codebase that builds on all of them." />

      <div className="grid g4">
        {ALL_OS.map((o, i) => {
          const isSource = sourceOs.includes(o) && sourceOs.length === 1;
          const on = targets.includes(o);
          const builders = localBuilders(o);
          return (
            <Reveal key={o} delay={i * .06}>
              <button className={`os-card ${OS_META[o].cls} ${on ? 'on' : ''}`} style={{ width: '100%' }} onClick={() => toggle(o)}>
                <span className="glow" />
                <span className="check"><Check size={15} strokeWidth={3} /></span>
                <div className="os-icon"><OsGlyph os={o} size={30} /></div>
                <div className="os-name">{OS_META[o].name}</div>
                <div className="muted small">{OS_META[o].blurb}</div>
                <div className="tag">
                  {isSource && <span className="chip small">Source OS</span>}
                  {o === hostOs && <span className="chip small"><Laptop size={12} /> This machine</span>}
                  {builders.length > 0
                    ? <span className="chip small"><Hammer size={12} /> Builds here</span>
                    : <span className="chip small"><Cloud size={12} /> Builds in CI</span>}
                </div>
              </button>
            </Reveal>
          );
        })}
      </div>

      <div className="grid g2" style={{ marginTop: 18 }}>
        <Reveal delay={.25} className="glass card">
          <h2 className="section"><FolderOutput size={16} /> Output folder</h2>
          <div className="path-box">
            <div className="mono" style={{ flex: 1 }}>{output}</div>
            <button className="btn" onClick={async () => { const p = await engine?.pickFolder('Choose where to write the ported project'); if (p) setOutput(p); }}>Change</button>
          </div>
          <div className="muted small" style={{ marginTop: 10 }}>Your original project is never modified.</div>
        </Reveal>
        <Reveal delay={.3} className="glass card">
          <div className="row">
            <div style={{ flex: 1 }}>
              <h2 className="section" style={{ marginBottom: 4 }}><Hammer size={16} /> Build after converting</h2>
              <div className="muted small">Compile and package every target that can build on this machine. The rest get a ready-to-run GitHub Actions workflow.</div>
            </div>
            <Switch on={buildAfter} onChange={setBuildAfter} label="Build after converting" />
          </div>
          <motion.div layout className="row" style={{ marginTop: 16, gap: 6, flexWrap: 'wrap' }}>
            {targets.map((o) => (
              <span key={o} className={`chip ${OS_META[o].cls}`}>
                <span style={{ color: 'var(--os)', display: 'flex' }}><OsGlyph os={o} size={13} /></span>
                {localBuilders(o).length ? localBuilders(o).map((t) => t.name).slice(0, 2).join(', ') : 'CI'}
              </span>
            ))}
          </motion.div>
        </Reveal>
      </div>

      <ActionBar>
        <span className="hint">{targets.length === 0 ? 'Pick at least one platform.' : `Porting to ${targets.map((t) => OS_META[t].name).join(', ')}.`}</span>
        <span className="spacer" />
        <button className="btn primary lg" disabled={targets.length === 0 || loading} onClick={next}>
          {loading ? 'Preparing questions…' : <>Continue <ArrowRight size={18} /></>}
        </button>
      </ActionBar>
    </div>
  );
}
