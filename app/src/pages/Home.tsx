import { Mic, Square, Settings, SlidersHorizontal } from 'lucide-react';
import type { CredentialsStatus, DictationSession, Preferences, StyleProfile } from '../lib/types';
import { MODE_LABEL } from '../lib/types';
import { formatDuration, formatTime } from '../lib/format';
import { formatHotkey } from '../components/ShortcutRecorder';

interface HomeProps {
  prefs: Preferences | null;
  styles: StyleProfile[];
  history: DictationSession[];
  credentials: CredentialsStatus | null;
  recording: boolean;
  busy: boolean;
  onStart: () => void;
  onStop: () => void;
  onOpenSettings: () => void;
  onActivateStyle: (id: string) => void;
}

export function Home({
  prefs,
  styles,
  history,
  credentials,
  recording,
  busy,
  onStart,
  onStop,
  onOpenSettings,
  onActivateStyle,
}: HomeProps) {
  const today = new Date();
  today.setHours(0, 0, 0, 0);
  const todayItems = history.filter(item => new Date(item.createdAt) >= today);
  const charsToday = todayItems.reduce((sum, item) => sum + item.finalText.length, 0);
  const durationToday = todayItems.reduce((sum, item) => sum + item.durationMs, 0);
  const activeStyle = styles.find(style => style.id === prefs?.activeStyleId);

  return (
    <div className="page home-page">
      <header className="page-header">
        <div>
          <p>语音输入</p>
          <h1>Typeless Lite</h1>
        </div>
        <button className="ghost-button" onClick={onOpenSettings}>
          <Settings size={16} />
          设置
        </button>
      </header>

      <section className="dictation-panel">
        <div>
          <span className="eyebrow">当前风格</span>
          <h2>{activeStyle?.name ?? '轻度润色'}</h2>
          <p>{formatHotkey(prefs?.hotkey ?? 'AltRight')} · {prefs?.hotkeyMode === 'toggle' ? '点击切换' : '按住说话'}</p>
        </div>
        <button className={recording ? 'record-button recording' : 'record-button'} disabled={busy && !recording} onClick={recording ? onStop : onStart}>
          {recording ? <Square size={22} /> : <Mic size={24} />}
          <span>{recording ? '停止录音' : '开始录音'}</span>
        </button>
      </section>

      <section className="style-strip">
        {styles.map(style => (
          <button
            key={style.id}
            className={style.id === prefs?.activeStyleId ? 'style-chip active' : 'style-chip'}
            onClick={() => onActivateStyle(style.id)}
          >
            <SlidersHorizontal size={14} />
            {style.name}
          </button>
        ))}
      </section>

      <section className="metrics-grid">
        <Metric label="今日字数" value={charsToday.toLocaleString()} />
        <Metric label="今日次数" value={String(todayItems.length)} />
        <Metric label="录音时长" value={formatDuration(durationToday)} />
        <Metric label="LLM" value={credentials?.llmConfigured ? '已配置' : '未配置'} warn={!credentials?.llmConfigured} />
      </section>

      <section className="recent-section">
        <div className="section-title">
          <h3>最近记录</h3>
          <span>{history.length} 条</span>
        </div>
        <div className="recent-list">
          {history.slice(0, 5).map(item => (
            <article key={item.id} className="recent-item">
              <div>
                <span>{formatTime(item.createdAt)}</span>
                <strong>{MODE_LABEL[item.mode]}</strong>
              </div>
              <p>{item.finalText || item.rawTranscript || '空记录'}</p>
            </article>
          ))}
          {history.length === 0 && <div className="empty-state">还没有历史记录。</div>}
        </div>
      </section>
    </div>
  );
}

function Metric({ label, value, warn }: { label: string; value: string; warn?: boolean }) {
  return (
    <div className={warn ? 'metric-card warn' : 'metric-card'}>
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}
