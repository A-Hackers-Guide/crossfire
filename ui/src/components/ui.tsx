import { motion } from 'framer-motion';
import type { ReactNode } from 'react';
import type { Os, Severity } from '../types';

export const OS_META: Record<Os, { name: string; cls: string; blurb: string }> = {
  windows: { name: 'Windows', cls: 'os-win', blurb: 'Windows 10 / 11 · x64 & ARM64' },
  macos: { name: 'macOS', cls: 'os-mac', blurb: 'macOS 12+ · Apple silicon & Intel' },
  linux: { name: 'Linux', cls: 'os-linux', blurb: 'Any modern distro · x64 & ARM64' },
  freebsd: { name: 'FreeBSD', cls: 'os-bsd', blurb: 'FreeBSD 14+ · amd64 & arm64' },
};

/** Simple, original glyphs for each OS (not vendor logos). */
export function OsGlyph({ os, size = 26 }: { os: Os; size?: number }) {
  const p = { width: size, height: size, viewBox: '0 0 32 32', fill: 'currentColor' } as const;
  switch (os) {
    case 'windows':
      return (
        <svg {...p}><rect x="4" y="4" width="11" height="11" rx="2" /><rect x="17" y="4" width="11" height="11" rx="2" />
          <rect x="4" y="17" width="11" height="11" rx="2" /><rect x="17" y="17" width="11" height="11" rx="2" /></svg>
      );
    case 'macos':
      return (
        <svg {...p} fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round">
          <path d="M11 11V8a3 3 0 1 0-3 3h3zm0 0h10m-10 0v10m10-10V8a3 3 0 1 1 3 3h-3zm0 0v10m0 0h3a3 3 0 1 1-3 3v-3zm0 0H11m0 0v3a3 3 0 1 1-3-3h3z" />
        </svg>
      );
    case 'linux':
      return (
        <svg {...p}>
          <path d="M16 3c-4 0-6 3.4-6 7.5 0 2-.6 3.3-1.8 5-1.6 2.3-3.2 5-3.2 8 0 .9.3 1.5 1 1.9 1.5.9 3 1.6 4 2.6h12c1-1 2.5-1.7 4-2.6.7-.4 1-1 1-1.9 0-3-1.6-5.7-3.2-8-1.2-1.7-1.8-3-1.8-5C22 6.4 20 3 16 3z" opacity=".95" />
          <ellipse cx="16" cy="20" rx="5" ry="6.5" fill="var(--bg)" opacity=".9" />
          <circle cx="13.6" cy="10" r="1.3" fill="var(--bg)" /><circle cx="18.4" cy="10" r="1.3" fill="var(--bg)" />
          <path d="M13.5 13h5l-2.5 2.2z" fill="var(--bg)" />
        </svg>
      );
    case 'freebsd':
      return (
        <svg {...p}>
          <path d="M16 6c5.5 0 10 4.3 10 10s-4.5 11-10 11S6 21.5 6 16 10.5 6 16 6z" />
          <path d="M5 3c3 0 6 2 6.5 5L8 10C6.5 8 5 6 5 3zm22 0c-3 0-6 2-6.5 5l3.5 2c1.5-2 3-4 3-7z" />
          <circle cx="12.5" cy="15" r="2" fill="var(--bg)" /><circle cx="19.5" cy="15" r="2" fill="var(--bg)" />
          <path d="M12 21c2.5 2 5.5 2 8 0" stroke="var(--bg)" strokeWidth="1.8" fill="none" strokeLinecap="round" />
        </svg>
      );
  }
}

export function Logo({ size = 38 }: { size?: number }) {
  return <img src={`${import.meta.env.BASE_URL}favicon.svg`} width={size} height={size} className="brand-mark" alt="" />;
}

export function PageHeader({ eyebrow, step, title, lede }: { eyebrow: string; step: number; title: ReactNode; lede?: ReactNode }) {
  return (
    <motion.div initial={{ opacity: 0, y: 10 }} animate={{ opacity: 1, y: 0 }} transition={{ duration: .45 }}>
      <div className="eyebrow"><b>Step {step}</b> · {eyebrow}</div>
      <h1 className="title">{title}</h1>
      {lede && <p className="lede">{lede}</p>}
    </motion.div>
  );
}

export function Reveal({ children, delay = 0, className, style }: { children: ReactNode; delay?: number; className?: string; style?: React.CSSProperties }) {
  return (
    <motion.div className={className} style={style} initial={{ opacity: 0, y: 14 }} animate={{ opacity: 1, y: 0 }}
      transition={{ duration: .5, delay, ease: [0.2, 0.8, 0.2, 1] }}>
      {children}
    </motion.div>
  );
}

export function Gauge({ value, size = 150, label }: { value: number; size?: number; label?: string }) {
  const r = size / 2 - 12;
  const c = 2 * Math.PI * r;
  return (
    <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`} style={{ flex: 'none' }}>
      <defs>
        <linearGradient id="gauge" x1="0" y1="1" x2="1" y2="0">
          <stop offset="0" stopColor="#ff7a18" /><stop offset=".5" stopColor="#ff2e63" /><stop offset="1" stopColor="#8b5cf6" />
        </linearGradient>
      </defs>
      <circle cx={size / 2} cy={size / 2} r={r} fill="none" stroke="var(--panel-2)" strokeWidth="12" />
      <motion.circle cx={size / 2} cy={size / 2} r={r} fill="none" stroke="url(#gauge)" strokeWidth="12" strokeLinecap="round"
        strokeDasharray={c} initial={{ strokeDashoffset: c }} animate={{ strokeDashoffset: c * (1 - value / 100) }}
        transition={{ duration: 1.4, ease: [0.2, 0.8, 0.2, 1] }} transform={`rotate(-90 ${size / 2} ${size / 2})`} />
      <text x="50%" y="48%" textAnchor="middle" dominantBaseline="middle" fill="var(--text)" style={{ fontSize: size * .24, fontWeight: 760, letterSpacing: '-.03em' }}>
        {Math.round(value)}%
      </text>
      {label && <text x="50%" y="66%" textAnchor="middle" fill="var(--text-3)" style={{ fontSize: 11.5, fontWeight: 560 }}>{label}</text>}
    </svg>
  );
}

export function Sev({ s }: { s: Severity }) {
  return <span className={`badge sev-${s}`}>{s.toUpperCase()}</span>;
}

export function Switch({ on, onChange, label }: { on: boolean; onChange: (b: boolean) => void; label?: string }) {
  return <button type="button" role="switch" aria-checked={on} aria-label={label} className={`switch ${on ? 'on' : ''}`} onClick={() => onChange(!on)} />;
}

export function ActionBar({ children }: { children: ReactNode }) {
  return (
    <motion.div className="actionbar" initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }} transition={{ duration: .4, delay: .15 }}>
      {children}
    </motion.div>
  );
}

export function Pipeline({ steps, current }: { steps: string[]; current: number }) {
  return (
    <div className="pipe glass">
      {steps.map((s, i) => (
        <div key={s} style={{ display: 'contents' }}>
          <div className={`pipe-step ${i < current ? 'done' : i === current ? 'active' : ''}`}>
            <span className="pdot">{i < current ? '✓' : i + 1}</span>{s}
          </div>
          {i < steps.length - 1 && <div className="pipe-line"><i style={{ width: i < current ? '100%' : '0%' }} /></div>}
        </div>
      ))}
    </div>
  );
}
