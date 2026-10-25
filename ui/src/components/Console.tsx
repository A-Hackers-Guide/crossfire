import { useEffect, useRef } from 'react';
import type { LogLine } from '../state';

export function Console({ lines, title, since = 0 }: { lines: LogLine[]; title: string; since?: number }) {
  const ref = useRef<HTMLDivElement>(null);
  const shown = lines.filter((l) => l.t >= since);
  useEffect(() => { ref.current?.scrollTo({ top: ref.current.scrollHeight, behavior: 'smooth' }); }, [shown.length]);
  const t0 = shown[0]?.t ?? Date.now();
  return (
    <div className="glass" style={{ overflow: 'hidden' }}>
      <div className="console-head">
        <span className="lights"><i /><i /><i /></span>
        <span className="small muted mono" style={{ marginLeft: 6 }}>{title}</span>
        <span className="spacer" />
        <span className="small muted">{shown.length} events</span>
      </div>
      <div className="console" ref={ref}>
        {shown.map((l, i) => (
          <div key={i} className={`l-${l.level}`}>
            <span className="ts">{((l.t - t0) / 1000).toFixed(2).padStart(6, ' ')}s</span>{l.message}{l.file ? <span className="ts"> · {l.file}</span> : null}
          </div>
        ))}
        {shown.length === 0 && <div className="l-info" style={{ opacity: .5 }}>Waiting for engine…</div>}
      </div>
    </div>
  );
}

export function Diff({ text }: { text: string }) {
  return (
    <div className="diff">
      {text.split('\n').map((l, i) => (
        <div key={i} className={l.startsWith('@@') ? 'hunk' : l.startsWith('+') ? 'add' : l.startsWith('-') ? 'del' : ''}>{l || ' '}</div>
      ))}
    </div>
  );
}
