import { ArrowRight, FolderOpen, FolderUp, Sparkles, Wand2 } from 'lucide-react';
import { useState } from 'react';
import { ALL_OS } from '../mock';
import { useStore } from '../state';
import { ActionBar, OS_META, OsGlyph, PageHeader, Reveal } from '../components/ui';

const FEATURES = [
  { t: 'Any project', d: 'C, C++, C#, Rust, Go, Swift, Obj-C, Python, JS, Java, plus Unity, Unreal and Godot.' },
  { t: '4 platforms', d: 'Windows, macOS, Linux and FreeBSD, each with its own native package.' },
  { t: 'Rules + AI', d: 'Exact rewrites for known APIs. Optional AI for the hard files.' },
  { t: 'Non-destructive', d: 'Your original folder is never touched. Output goes to a new folder.' },
];

export function SourceView() {
  const { engine, source, setSource, sourceOs, setSourceOs, go } = useStore();
  const [over, setOver] = useState(false);

  const pick = async () => {
    const p = await engine?.pickFolder('Choose your project folder');
    if (p) setSource(p);
  };
  const toggleOs = (o: (typeof ALL_OS)[number]) =>
    setSourceOs(sourceOs.includes(o) ? sourceOs.filter((x) => x !== o) : [...sourceOs, o]);

  return (
    <div className="page-inner">
      <PageHeader step={1} eyebrow="Source"
        title={<>Bring your code.<br /><em>Ship it everywhere.</em></>}
        lede="Point Crossfire at any project. It maps every OS-specific call, asks a few questions, then writes and builds a version for each platform you pick." />

      <Reveal delay={.08}>
        {!source ? (
          <div className={`dropzone ${over ? 'over' : ''}`} onClick={pick}
            onDragOver={(e) => { e.preventDefault(); setOver(true); }} onDragLeave={() => setOver(false)}
            onDrop={(e) => {
              e.preventDefault(); setOver(false);
              const file = e.dataTransfer.files[0] as File & { path?: string };
              if (file?.path) setSource(file.path); else pick();
            }}>
            <div>
              <div className="drop-orb" style={{ margin: '0 auto 18px' }}><FolderUp size={40} strokeWidth={1.6} /></div>
              <div className="drop-title">Drop a project folder here</div>
              <div className="muted" style={{ marginTop: 6 }}>or click to browse · a repo root, a game folder, an Xcode/VS solution</div>
            </div>
          </div>
        ) : (
          <div className="glass card">
            <div className="path-box">
              <FolderOpen size={22} color="var(--fire-2)" />
              <div style={{ minWidth: 0, flex: 1 }}>
                <div style={{ fontWeight: 650 }}>{source.split(/[\\/]/).filter(Boolean).pop()}</div>
                <div className="mono">{source}</div>
              </div>
              <button className="btn" onClick={pick}>Change</button>
            </div>

            <h2 className="section" style={{ marginTop: 26 }}>Which OS is this code written for?
              <span className="count">Optional — Crossfire detects it during analysis</span></h2>
            <div className="row" style={{ flexWrap: 'wrap', gap: 8 }}>
              <button className={`chip ${sourceOs.length === 0 ? 'on' : ''}`} onClick={() => setSourceOs([])}>
                <Sparkles size={14} /> Detect automatically
              </button>
              {ALL_OS.map((o) => (
                <button key={o} className={`chip ${sourceOs.includes(o) ? 'on' : ''} ${OS_META[o].cls}`} onClick={() => toggleOs(o)}>
                  <span style={{ color: 'var(--os)', display: 'flex' }}><OsGlyph os={o} size={14} /></span>{OS_META[o].name}
                </button>
              ))}
            </div>
          </div>
        )}
      </Reveal>

      <div className="grid g4" style={{ marginTop: 22 }}>
        {FEATURES.map((f, i) => (
          <Reveal key={f.t} delay={.14 + i * .05} className="glass card">
            <div style={{ fontWeight: 680, marginBottom: 4 }}>{f.t}</div>
            <div className="muted small">{f.d}</div>
          </Reveal>
        ))}
      </div>

      <ActionBar>
        <Wand2 size={18} color="var(--fire-2)" />
        <span className="hint">{source ? 'Ready to map every platform dependency in your project.' : 'Choose a project to begin.'}</span>
        <span className="spacer" />
        <button className="btn primary lg" disabled={!source} onClick={() => go('analyze')}>
          Analyze project <ArrowRight size={18} />
        </button>
      </ActionBar>
    </div>
  );
}
