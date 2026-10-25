import { FileText, FolderOpen, PartyPopper, Plus } from 'lucide-react';
import { useStore } from '../state';
import { OS_META, OsGlyph, Reveal, Sev } from '../components/ui';

export function ReportView() {
  const { scan, convertResult, buildResult, targets, engine, reset } = useStore();
  if (!convertResult) return null;
  const built = buildResult?.results.filter((r) => r.status === 'success').length ?? 0;

  return (
    <div className="page-inner">
      <Reveal className="glass hero-done" style={{ marginBottom: 16 }}>
        <div className="burst" />
        <div className="hero-badge"><PartyPopper size={40} /></div>
        <h1 className="title" style={{ fontSize: 36 }}><em>{scan?.name}</em> now targets {targets.length} platform{targets.length === 1 ? '' : 's'}</h1>
        <p className="lede" style={{ margin: '0 auto 22px' }}>
          {convertResult.changes.filter((c) => c.action !== "copied").length} files changed · {built} built locally · {convertResult.manualReview.length} item{convertResult.manualReview.length === 1 ? '' : 's'} to review
        </p>
        <div className="row" style={{ justifyContent: 'center', gap: 10, flexWrap: 'wrap' }}>
          {targets.map((o) => (
            <span key={o} className={`chip on ${OS_META[o].cls}`}><span style={{ color: 'var(--os)', display: 'flex' }}><OsGlyph os={o} size={14} /></span>{OS_META[o].name}</span>
          ))}
        </div>
        <div className="row" style={{ justifyContent: 'center', gap: 10, marginTop: 26 }}>
          <button className="btn lg" onClick={() => engine?.openPath(convertResult.reportPath)}><FileText size={18} /> Open full report</button>
          <button className="btn primary lg" onClick={() => engine?.openPath(convertResult.output)}><FolderOpen size={18} /> Open ported project</button>
        </div>
      </Reveal>

      {convertResult.manualReview.length > 0 && (
        <Reveal delay={.1} className="glass" style={{ overflow: 'hidden', marginBottom: 16 }}>
          <div className="toolbar"><h2 className="section" style={{ margin: 0 }}>Review these by hand <span className="count">Lower-confidence changes</span></h2></div>
          {convertResult.manualReview.map((f) => (
            <div key={f.id} className="finding" style={{ cursor: 'default' }}>
              <Sev s={f.severity} />
              <div><div className="api">{f.api}</div><div className="small" style={{ color: 'var(--text-2)' }}>{f.message}</div><div className="loc">{f.file}:{f.line}</div></div>
              <span />
            </div>
          ))}
        </Reveal>
      )}

      <Reveal delay={.15} className="glass card">
        <h2 className="section">Next steps</h2>
        <ol style={{ margin: 0, paddingLeft: 20, color: 'var(--text-2)', lineHeight: 1.9 }}>
          <li>Open the ported project and run it on this machine.</li>
          <li>Check each item in the review list above.</li>
          <li>Push the project to GitHub. The generated workflow builds Windows, macOS, Linux and FreeBSD.</li>
          <li>Download the packaged builds (.exe, .app/.dmg, AppImage, pkg) from the workflow run.</li>
        </ol>
        <button className="btn" style={{ marginTop: 18 }} onClick={reset}><Plus size={16} /> Port another project</button>
      </Reveal>
    </div>
  );
}
