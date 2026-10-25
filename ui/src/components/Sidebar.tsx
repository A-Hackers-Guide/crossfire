import { motion } from 'framer-motion';
import { Check, Cpu, Moon, Settings, Sun } from 'lucide-react';
import { STEPS, useStore } from '../state';
import { Logo } from './ui';

export function Sidebar({ onToolchains }: { onToolchains: () => void }) {
  const { step, go, reached, engine, theme, setTheme, setSettingsOpen, scan, targets, convertResult, buildResult } = useStore();
  const idx = STEPS.findIndex((s) => s.id === step);

  const meta: Partial<Record<string, string>> = {
    analyze: scan ? `${Math.round(scan.portability)}%` : undefined,
    targets: targets.length ? `${targets.length} OS` : undefined,
    convert: convertResult ? `${convertResult.changes.filter((c) => c.action !== 'copied').length} files` : undefined,
    build: buildResult ? `${buildResult.results.filter((r) => r.status === 'success').length}/${buildResult.results.length}` : undefined,
  };

  return (
    <aside className="sidebar">
      <div className="brand">
        <Logo />
        <div>
          <div className="brand-name">Crossfire</div>
          <div className="brand-sub">Port anything, everywhere</div>
        </div>
      </div>

      <div className="nav-label">Workflow</div>
      {STEPS.map((s, i) => {
        const active = s.id === step;
        const done = i < idx && reached.has(s.id);
        return (
          <button key={s.id} className={`nav-item ${active ? 'active' : ''} ${done ? 'done' : ''}`}
            disabled={!reached.has(s.id)} onClick={() => go(s.id)}>
            {active && <motion.span layoutId="nav-bg" className="nav-active-bg" transition={{ type: 'spring', stiffness: 420, damping: 36 }} />}
            <span className="nav-step">{done ? <Check size={13} strokeWidth={3} /> : i + 1}</span>
            {s.label}
            {meta[s.id] && <span className="nav-meta">{meta[s.id]}</span>}
          </button>
        );
      })}

      <div className="nav-label">Workspace</div>
      <button className="nav-item" onClick={onToolchains}><span className="nav-step"><Cpu size={13} /></span>Toolchains</button>
      <button className="nav-item" onClick={() => setSettingsOpen(true)}><span className="nav-step"><Settings size={13} /></span>Settings</button>

      <div className="sidebar-foot">
        <button className="nav-item" onClick={() => setTheme(theme === 'dark' ? 'light' : 'dark')}>
          <span className="nav-step">{theme === 'dark' ? <Sun size={13} /> : <Moon size={13} />}</span>
          {theme === 'dark' ? 'Light mode' : 'Dark mode'}
        </button>
        <div className="engine-pill">
          <span className={`dot ${engine?.kind === 'demo' ? 'demo' : ''}`} />
          {engine ? (engine.kind === 'demo' ? 'Demo engine (browser)' : `Engine ready · ${engine.kind === 'tauri' ? 'native' : 'sidecar'}`) : 'Starting engine…'}
        </div>
      </div>
    </aside>
  );
}
