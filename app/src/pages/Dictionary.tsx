import { Plus, RefreshCw, Trash2 } from 'lucide-react';
import { useState } from 'react';
import type { CorrectionRule, DictionaryEntry } from '../lib/types';

interface DictionaryProps {
  entries: DictionaryEntry[];
  rules: CorrectionRule[];
  onRefresh: () => void;
  onAddEntry: (phrase: string, note: string | null) => void;
  onRemoveEntry: (id: string) => void;
  onToggleEntry: (id: string, enabled: boolean) => void;
  onAddRule: (pattern: string, replacement: string) => void;
  onRemoveRule: (id: string) => void;
  onToggleRule: (id: string, enabled: boolean) => void;
}

export function Dictionary(props: DictionaryProps) {
  const [phrase, setPhrase] = useState('');
  const [note, setNote] = useState('');
  const [pattern, setPattern] = useState('');
  const [replacement, setReplacement] = useState('');

  const addEntry = () => {
    if (!phrase.trim()) return;
    props.onAddEntry(phrase, note || null);
    setPhrase('');
    setNote('');
  };

  const addRule = () => {
    if (!pattern.trim()) return;
    props.onAddRule(pattern, replacement);
    setPattern('');
    setReplacement('');
  };

  return (
    <div className="page dictionary-page">
      <header className="page-header">
        <div>
          <p>词典</p>
          <h1>专有名词和纠错</h1>
        </div>
        <button className="ghost-button" onClick={props.onRefresh}><RefreshCw size={15} />刷新</button>
      </header>

      <section className="editor-card">
        <h3>添加词条</h3>
        <div className="inline-form">
          <input value={phrase} placeholder="词条，例如 Claude Code" onChange={event => setPhrase(event.target.value)} onKeyDown={event => { if (event.key === 'Enter') addEntry(); }} />
          <input value={note} placeholder="备注，可选" onChange={event => setNote(event.target.value)} />
          <button className="primary" onClick={addEntry}><Plus size={15} />添加</button>
        </div>
        <div className="chip-list">
          {props.entries.map(entry => (
            <span key={entry.id} className={entry.enabled ? 'dict-chip' : 'dict-chip disabled'}>
              <button onClick={() => props.onToggleEntry(entry.id, !entry.enabled)}>{entry.phrase}</button>
              <small>{entry.hits}</small>
              <button className="chip-close" onClick={() => props.onRemoveEntry(entry.id)}><Trash2 size={12} /></button>
            </span>
          ))}
          {props.entries.length === 0 && <div className="empty-state">还没有词条。</div>}
        </div>
      </section>

      <section className="editor-card">
        <h3>纠错规则</h3>
        <div className="inline-form">
          <input value={pattern} placeholder="错误写法" onChange={event => setPattern(event.target.value)} />
          <input value={replacement} placeholder="替换为" onChange={event => setReplacement(event.target.value)} onKeyDown={event => { if (event.key === 'Enter') addRule(); }} />
          <button className="primary" onClick={addRule}><Plus size={15} />添加</button>
        </div>
        <div className="chip-list">
          {props.rules.map(rule => (
            <span key={rule.id} className={rule.enabled ? 'dict-chip rule' : 'dict-chip rule disabled'}>
              <button onClick={() => props.onToggleRule(rule.id, !rule.enabled)}>{rule.pattern} → {rule.replacement}</button>
              <button className="chip-close" onClick={() => props.onRemoveRule(rule.id)}><Trash2 size={12} /></button>
            </span>
          ))}
          {props.rules.length === 0 && <div className="empty-state">还没有纠错规则。</div>}
        </div>
      </section>
    </div>
  );
}
