import { AnimatePresence, motion } from 'framer-motion';
import { CheckCircle2, CircleDashed, RefreshCw, X } from 'lucide-react';
import { useState, type ReactNode } from 'react';
import { useStore } from '../state';
import { OS_META, OsGlyph, Switch } from './ui';

function Modal({ open, onClose, children }: { open: boolean; onClose: () => void; children: ReactNode }) {
  return (
    <AnimatePresence>
      {open && (
        <motion.div className="overlay" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} onClick={onClose}>
          <motion.div className="glass modal" initial={{ opacity: 0, y: 20, scale: .97 }} animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: 10, scale: .98 }} transition={{ type: 'spring', stiffness: 380, damping: 32 }} onClick={(e) => e.stopPropagation()}>
            <button className="btn icon ghost" style={{ position: 'absolute', top: 18, right: 18 }} onClick={onClose} aria-label="Close"><X size={18} /></button>
            {children}
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

export function SettingsDialog() {
  const { settingsOpen, setSettingsOpen, ai, setAi, theme, setTheme } = useStore();
  const [tab, setTab] = useState<'ai' | 'appearance'>('ai');
  return (
    <Modal open={settingsOpen} onClose={() => setSettingsOpen(false)}>
      <h3>Settings</h3>
      <div className="muted small" style={{ marginBottom: 18 }}>Saved on this computer only.</div>
      <div className="tabs" style={{ marginBottom: 20 }}>
        <button className={tab === 'ai' ? 'on' : ''} onClick={() => setTab('ai')}>AI model</button>
        <button className={tab === 'appearance' ? 'on' : ''} onClick={() => setTab('appearance')}>Appearance</button>
      </div>
      {tab === 'ai' ? (
        <div className="grid" style={{ gap: 16 }}>
          <div className="row">
            <div style={{ flex: 1 }}><div style={{ fontWeight: 650 }}>AI-assisted rewrites</div>
              <div className="muted small">Sends only the files with blocker findings to your provider.</div></div>
            <Switch on={ai.enabled} onChange={(b) => setAi({ ...ai, enabled: b })} label="AI-assisted rewrites" />
          </div>
          <label className="field">Provider
            <select className="input" value={ai.provider} onChange={(e) => setAi({ ...ai, provider: e.target.value as typeof ai.provider })}>
              <option value="anthropic">Anthropic (Claude)</option>
              <option value="openaiCompatible">OpenAI-compatible endpoint (local or hosted)</option>
            </select>
          </label>
          {ai.provider === 'openaiCompatible' && (
            <label className="field">Base URL
              <input className="input mono" placeholder="http://localhost:11434/v1" value={ai.baseUrl ?? ''} onChange={(e) => setAi({ ...ai, baseUrl: e.target.value })} />
            </label>
          )}
          <label className="field">Model
            <input className="input mono" value={ai.model} onChange={(e) => setAi({ ...ai, model: e.target.value })} />
          </label>
          <label className="field">API key
            <input className="input mono" type="password" autoComplete="off" placeholder="Paste your key" value={ai.apiKey} onChange={(e) => setAi({ ...ai, apiKey: e.target.value })} />
          </label>
        </div>
      ) : (
        <div className="grid g2">
          {(['dark', 'light'] as const).map((t) => (
            <button key={t} className={`opt ${theme === t ? 'on' : ''}`} onClick={() => setTheme(t)}>
              <div className="ol"><span className="radio" />{t === 'dark' ? 'Dark' : 'Light'}</div>
              <div className="od">{t === 'dark' ? 'Deep ember glow' : 'Soft daylight'}</div>
            </button>
          ))}
        </div>
      )}
    </Modal>
  );
}

export function ToolchainsDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const { toolchains, refreshToolchains, hostOs } = useStore();
  const [busy, setBusy] = useState(false);
  return (
    <Modal open={open} onClose={onClose}>
      <h3>Toolchains</h3>
      <div className="muted small" style={{ marginBottom: 18 }}>What this {OS_META[hostOs].name} machine can build. Missing ones fall back to CI.</div>
      <div className="grid" style={{ gap: 8 }}>
        {toolchains.map((t) => (
          <div key={t.id} className="row" style={{ padding: '12px 14px', borderRadius: 12, background: 'var(--panel)', border: '1px solid var(--stroke)' }}>
            {t.found ? <CheckCircle2 size={18} color="var(--ok)" /> : <CircleDashed size={18} color="var(--text-3)" />}
            <div style={{ flex: 1, minWidth: 0 }}>
              <div style={{ fontWeight: 620 }}>{t.name} {t.version && <span className="muted small">{t.version}</span>}</div>
              <div className="muted small mono" style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{t.found ? t.path : t.hint}</div>
            </div>
            <div className="row" style={{ gap: 4 }}>
              {t.targets.map((o) => <span key={o} className={OS_META[o].cls} title={OS_META[o].name} style={{ color: 'var(--os)', display: 'flex', opacity: t.found ? 1 : .35 }}><OsGlyph os={o} size={15} /></span>)}
            </div>
          </div>
        ))}
      </div>
      <button className="btn" style={{ marginTop: 18 }} disabled={busy} onClick={async () => { setBusy(true); await refreshToolchains(); setBusy(false); }}>
        <RefreshCw size={16} /> {busy ? 'Checking…' : 'Check again'}
      </button>
    </Modal>
  );
}
