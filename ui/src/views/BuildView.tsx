import { ArrowRight, CheckCircle2, Cloud, FlaskConical, Hammer, Loader2, Package, RotateCcw, XCircle } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { useStore } from '../state';
import type { BuildTargetResult, Os } from '../types';
import { Console } from '../components/Console';
import { ActionBar, OS_META, OsGlyph, PageHeader, Reveal } from '../components/ui';

const STATUS: Record<BuildTargetResult['status'] | 'running', { label: string; Icon: typeof Hammer }> = {
  success: { label: 'Built', Icon: CheckCircle2 },
  failed: { label: 'Failed', Icon: XCircle },
  ci: { label: 'Builds in CI', Icon: Cloud },
  skipped: { label: 'Skipped', Icon: XCircle },
  running: { label: 'Building…', Icon: Loader2 },
};

export function BuildView() {
  const { engine, convertResult, targets, buildResult, setBuildResult, log, toolchains, hostOs, answers, go } = useStore();
  const [since, setSince] = useState(0);
  const [running, setRunning] = useState(false);
  const [gptk, setGptk] = useState<string | null>(null);
  const started = useRef(false);
  const hasGptk = hostOs === 'macos' && toolchains.some((t) => t.id === 'gptk' && t.found);

  const run = async () => {
    if (!engine || !convertResult) return;
    setRunning(true); setBuildResult(null); setSince(Date.now());
    try {
      setBuildResult(await engine.build({ project: convertResult.output, targets, release: true, package: Array.isArray(answers.packages) && answers.packages.length > 0 }));
    } finally { setRunning(false); }
  };

  useEffect(() => {
    if (!buildResult && engine && !started.current) { started.current = true; run(); }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [engine]);

  const testWin = async () => {
    if (!engine) return;
    const winArt = buildResult?.results.find((r) => r.target === 'windows')?.artifacts.find((a) => a.endsWith('.exe'));
    const exe = winArt ?? await engine.pickFile('Choose a Windows .exe to test', ['exe']);
    if (!exe) return;
    setGptk('running'); setSince(Date.now());
    try { setGptk(await engine.testWithGptk(exe)); } catch (e) { setGptk(String(e)); }
  };

  const resultFor = (o: Os) => buildResult?.results.find((r) => r.target === o);

  // While the build runs, infer each target's state from its "[os] …" progress lines
  const liveStatus = (o: Os): BuildTargetResult['status'] | 'running' => {
    const lines = log.filter((l) => l.stage === 'build' && l.t >= since && l.message.startsWith(`[${o}]`));
    const end = [...lines].reverse().find((l) => l.level !== 'info');
    if (!end) return 'running';
    return end.level === 'success' ? 'success' : end.level === 'error' ? 'failed' : 'ci';
  };

  return (
    <div className="page-inner">
      <PageHeader step={6} eyebrow="Build" title={running ? <>Building <em>everywhere</em></> : <>Build <em>results</em></>}
        lede="Targets with a local toolchain build right here. The rest build on GitHub Actions with the workflow Crossfire generated." />

      <div className="grid g4" style={{ marginBottom: 16 }}>
        {targets.map((o, i) => {
          const r = resultFor(o);
          const st = r ? r.status : running ? liveStatus(o) : 'skipped';
          const { label, Icon } = STATUS[st];
          return (
            <Reveal key={o} delay={i * .05} className={`glass target-card ${OS_META[o].cls}`}>
              <div className="row">
                <div className="os-icon" style={{ width: 44, height: 44, borderRadius: 13, display: 'grid', placeItems: 'center', background: 'var(--panel-2)', color: 'var(--os)' }}>
                  <OsGlyph os={o} size={24} />
                </div>
                <div><div style={{ fontWeight: 700 }}>{OS_META[o].name}</div>
                  <span className={`status ${st}`}><Icon size={14} className={st === 'running' ? 'spin' : ''} style={st === 'running' ? { animation: 'spin 1s linear infinite' } : undefined} />{label}</span></div>
              </div>
              {r?.artifacts.map((a) => (
                <div key={a} className="artifact" onClick={() => engine?.openPath(a)} style={{ cursor: 'pointer' }}><Package size={14} /><span>{a}</span></div>
              ))}
              {r?.reason && <div className="small muted">{r.reason}</div>}
            </Reveal>
          );
        })}
      </div>

      {hasGptk && targets.includes('windows') && (
        <Reveal delay={.2} className="glass card" style={{ marginBottom: 16 }}>
          <div className="row">
            <FlaskConical size={22} color="var(--win)" />
            <div style={{ flex: 1 }}>
              <div style={{ fontWeight: 680 }}>Test the Windows build on this Mac</div>
              <div className="muted small">Runs the .exe through Apple's Game Porting Toolkit (Wine + D3DMetal) so you can try it without a PC.</div>
              {gptk && gptk !== 'running' && <div className="small" style={{ marginTop: 6, color: 'var(--text-2)' }}>{gptk}</div>}
            </div>
            <button className="btn" disabled={gptk === 'running'} onClick={testWin}>{gptk === 'running' ? 'Running…' : 'Run with GPTK'}</button>
          </div>
        </Reveal>
      )}

      <Console lines={log.filter((l) => l.stage === 'build' || l.stage === 'test')} since={since} title="crossfire build" />

      {buildResult?.ciWorkflow && (
        <Reveal className="glass card" style={{ marginTop: 16 }}>
          <div className="row"><Cloud size={18} color="var(--info)" />
            <div style={{ flex: 1 }}><div style={{ fontWeight: 650 }}>CI workflow ready</div>
              <div className="muted small mono">{buildResult.ciWorkflow}</div>
              <div className="muted small">Push the output folder to GitHub and every platform builds automatically, including FreeBSD in a VM.</div></div>
          </div>
        </Reveal>
      )}

      <ActionBar>
        <button className="btn ghost" disabled={running} onClick={run}><RotateCcw size={16} /> Rebuild</button>
        <span className="spacer" />
        <button className="btn primary lg" disabled={running} onClick={() => go('report')}>Finish <ArrowRight size={18} /></button>
      </ActionBar>
    </div>
  );
}
