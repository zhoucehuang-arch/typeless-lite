import { Copy, RefreshCw, Trash2 } from 'lucide-react';
import { useMemo, useState } from 'react';
import { MODE_LABEL, type DictationSession, type PolishMode } from '../lib/types';
import { formatDuration, formatTime } from '../lib/format';

interface HistoryProps {
  items: DictationSession[];
  onRefresh: () => void;
  onDelete: (id: string) => void;
  onClear: () => void;
}

export function History({ items, onRefresh, onDelete, onClear }: HistoryProps) {
  const [filter, setFilter] = useState<'all' | PolishMode>('all');
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const filtered = useMemo(() => filter === 'all' ? items : items.filter(item => item.mode === filter), [items, filter]);
  const selected = filtered.find(item => item.id === selectedId) ?? filtered[0] ?? null;

  const copy = async () => {
    if (selected) await navigator.clipboard.writeText(selected.finalText);
  };

  return (
    <div className="page history-page">
      <header className="page-header">
        <div>
          <p>历史记录</p>
          <h1>找回每次输入</h1>
        </div>
        <div className="header-actions">
          <button className="ghost-button" onClick={onRefresh}><RefreshCw size={15} />刷新</button>
          <button className="ghost-button danger" onClick={onClear}><Trash2 size={15} />清空</button>
        </div>
      </header>
      <div className="filter-row">
        {(['all', 'raw', 'light', 'structured', 'formal'] as Array<'all' | PolishMode>).map(mode => (
          <button key={mode} className={filter === mode ? 'filter active' : 'filter'} onClick={() => setFilter(mode)}>
            {mode === 'all' ? '全部' : MODE_LABEL[mode]}
          </button>
        ))}
      </div>
      <div className="split-view">
        <aside className="list-pane">
          {filtered.map(item => (
            <button key={item.id} className={selected?.id === item.id ? 'history-row active' : 'history-row'} onClick={() => setSelectedId(item.id)}>
              <span>{formatTime(item.createdAt)} · {MODE_LABEL[item.mode]}</span>
              <strong>{item.finalText || item.rawTranscript || '空记录'}</strong>
            </button>
          ))}
          {filtered.length === 0 && <div className="empty-state">没有匹配记录。</div>}
        </aside>
        <section className="detail-pane">
          {selected ? (
            <>
              <div className="detail-toolbar">
                <span>{formatTime(selected.createdAt)} · {formatDuration(selected.durationMs)}</span>
                <div>
                  <button className="ghost-button" onClick={() => void copy()}><Copy size={15} />复制</button>
                  <button className="ghost-button danger" onClick={() => onDelete(selected.id)}><Trash2 size={15} />删除</button>
                </div>
              </div>
              <div className="text-columns">
                <TextBox title="原始转写" text={selected.rawTranscript} />
                <TextBox title={MODE_LABEL[selected.mode]} text={selected.finalText} accent />
              </div>
              <div className="meta-row">
                <span>插入状态：{selected.insertStatus}</span>
                {selected.errorCode && <span>错误：{selected.errorCode}</span>}
                <span>词典命中：{selected.dictionaryHitCount}</span>
              </div>
            </>
          ) : (
            <div className="empty-state">选择一条记录查看详情。</div>
          )}
        </section>
      </div>
    </div>
  );
}

function TextBox({ title, text, accent }: { title: string; text: string; accent?: boolean }) {
  return (
    <section className={accent ? 'text-box accent' : 'text-box'}>
      <h3>{title}</h3>
      <p>{text || '无内容'}</p>
    </section>
  );
}
