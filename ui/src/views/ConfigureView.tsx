import { ArrowRight, KeyRound, Sparkles, Zap } from 'lucide-react';
import { useEffect } from 'react';
import { useStore } from '../state';
import type { Question } from '../types';
import { ActionBar, PageHeader, Reveal, Switch } from '../components/ui';

export function ConfigureView() {
  const { questions, answers, setAnswer, go, ai, setSettingsOpen } = useStore();

  // Seed answers with each question's default the first time we see it
  useEffect(() => {
    questions.forEach((q) => { if (answers[q.id] === undefined) setAnswer(q.id, q.default); });
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [questions]);

  const visible = questions.filter((q) => !q.dependsOn || answers[q.dependsOn.question] === q.dependsOn.value
    || (Array.isArray(answers[q.dependsOn.question]) && (answers[q.dependsOn.question] as string[]).includes(q.dependsOn.value)));
  const wantsAi = answers.useAi === true;
  const needsKey = wantsAi && !ai.apiKey;

  const useRecommended = () => questions.forEach((q) => {
    if (q.kind === 'single') { const r = q.options.find((o) => o.recommended); if (r) setAnswer(q.id, r.value); }
    else setAnswer(q.id, q.default);
  });

  return (
    <div className="page-inner">
      <PageHeader step={4} eyebrow="Configure" title={<>A few <em>quick questions</em></>}
        lede="These come from what Crossfire found in your code. The recommended answers are a safe default." />

      <Reveal delay={.04} style={{ marginBottom: 16 }}>
        <button className="btn" onClick={useRecommended}><Zap size={16} /> Use all recommended answers</button>
      </Reveal>

      <div className="grid" style={{ gap: 14 }}>
        {visible.map((q, i) => (
          <Reveal key={q.id} delay={.06 + i * .04} className="glass q">
            <QuestionCard q={q} value={answers[q.id]} onChange={(v) => setAnswer(q.id, v)} />
            {q.id === 'useAi' && wantsAi && (
              <div className="row" style={{ marginTop: 14, padding: '12px 14px', borderRadius: 12, background: 'var(--panel)' }}>
                <KeyRound size={16} color={needsKey ? 'var(--warn)' : 'var(--ok)'} />
                <span className="small" style={{ flex: 1 }}>
                  {needsKey ? 'Add an API key in Settings to enable AI rewrites. Without one, Crossfire uses rules only.' : `Using ${ai.model} via ${ai.provider === 'anthropic' ? 'Anthropic' : ai.baseUrl}.`}
                </span>
                <button className="btn" onClick={() => setSettingsOpen(true)}>{needsKey ? 'Add key' : 'Change'}</button>
              </div>
            )}
          </Reveal>
        ))}
      </div>

      <ActionBar>
        <Sparkles size={18} color="var(--fire-2)" />
        <span className="hint">You can come back and change these after seeing the result.</span>
        <span className="spacer" />
        <button className="btn primary lg" onClick={() => go('convert')}>Convert project <ArrowRight size={18} /></button>
      </ActionBar>
    </div>
  );
}

function QuestionCard({ q, value, onChange }: { q: Question; value: unknown; onChange: (v: string | string[] | boolean) => void }) {
  if (q.kind === 'toggle') {
    return (
      <div className="row">
        <div style={{ flex: 1 }}><div className="q-title">{q.title}</div><div className="q-desc" style={{ marginBottom: 0 }}>{q.description}</div></div>
        <Switch on={value === true} onChange={onChange} label={q.title} />
      </div>
    );
  }
  if (q.kind === 'text') {
    return (
      <>
        <div className="q-title">{q.title}</div><div className="q-desc">{q.description}</div>
        <input className="input" value={(value as string) ?? ''} onChange={(e) => onChange(e.target.value)} />
      </>
    );
  }
  const multi = q.kind === 'multi';
  const sel = multi ? ((value as string[]) ?? []) : [value as string];
  return (
    <>
      <div className="q-title">{q.title}</div>
      <div className="q-desc">{q.description}</div>
      <div className="opts">
        {q.options.map((o) => {
          const on = sel.includes(o.value);
          return (
            <button key={o.value} className={`opt ${multi ? 'multi' : ''} ${on ? 'on' : ''}`}
              onClick={() => onChange(multi ? (on ? sel.filter((x) => x !== o.value) : [...sel, o.value]) : o.value)}>
              <div className="ol"><span className="radio" />{o.label}{o.recommended && <span className="badge rec">Recommended</span>}</div>
              <div className="od">{o.description}</div>
            </button>
          );
        })}
      </div>
    </>
  );
}
