import { RotateCcw, Save } from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';
import { MODE_LABEL, type Preferences, type StyleProfile } from '../lib/types';

interface StylesProps {
  prefs: Preferences | null;
  styles: StyleProfile[];
  onActivate: (id: string) => void;
  onSave: (style: StyleProfile) => void;
  onReset: (id: string) => void;
}

export function Styles({ prefs, styles, onActivate, onSave, onReset }: StylesProps) {
  const selected = useMemo(
    () => styles.find(style => style.id === prefs?.activeStyleId) ?? styles[0] ?? null,
    [styles, prefs],
  );
  const [draft, setDraft] = useState<StyleProfile | null>(selected);

  useEffect(() => {
    setDraft(selected);
  }, [selected?.id, selected?.prompt]);

  return (
    <div className="page styles-page">
      <header className="page-header">
        <div>
          <p>润色风格</p>
          <h1>切换和编辑提示词</h1>
        </div>
      </header>
      <div className="styles-layout">
        <aside className="style-list">
          {styles.map(style => (
            <button key={style.id} className={style.id === prefs?.activeStyleId ? 'style-card active' : 'style-card'} onClick={() => onActivate(style.id)}>
              <strong>{style.name}</strong>
              <span>{MODE_LABEL[style.mode]}</span>
            </button>
          ))}
        </aside>
        <section className="style-editor">
          {draft ? (
            <>
              <div className="detail-toolbar">
                <span>{draft.name} · {MODE_LABEL[draft.mode]}</span>
                <div>
                  <button className="ghost-button" onClick={() => onReset(draft.id)}><RotateCcw size={15} />重置</button>
                  <button className="primary" onClick={() => onSave(draft)}><Save size={15} />保存</button>
                </div>
              </div>
              <label>
                名称
                <input value={draft.name} onChange={event => setDraft({ ...draft, name: event.target.value })} />
              </label>
              <label className="prompt-label">
                系统提示词
                <textarea value={draft.prompt} onChange={event => setDraft({ ...draft, prompt: event.target.value })} />
              </label>
            </>
          ) : (
            <div className="empty-state">没有可用风格。</div>
          )}
        </section>
      </div>
    </div>
  );
}
