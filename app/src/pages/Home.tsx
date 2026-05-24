import type { ReactNode } from 'react';
import { BarChart3, CheckCircle2, Clock, Hash, Mic, Settings, SlidersHorizontal, Sparkles, Square, Zap } from 'lucide-react';
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
  const weekly = weekBuckets(history);
  const activeStyle = styles.find(style => style.id === prefs?.activeStyleId);

  return (
    <div className="page home-page">
      <header className="page-header">
        <div>
          <p>Overview</p>
          <h1>语音输入</h1>
        </div>
        <button className="ghost-button" onClick={onOpenSettings}>
          <Settings size={16} />
          设置
        </button>
      </header>

      <section className="home-top-grid">
        <div className="home-main-stack">
          <section className="status-grid">
            <ProviderCard
              icon={<Mic size={18} />}
              kind="ASR"
              name="SenseVoice Small"
              status="本地模型已就绪"
              ready
            />
            <ProviderCard
              icon={<Sparkles size={18} />}
              kind="LLM"
              name={prefs?.llmModel || '未选择模型'}
              status={credentials?.llmConfigured ? '已配置' : '未配置'}
              ready={Boolean(credentials?.llmConfigured)}
            />
          </section>

          <section className="dictation-panel">
            <div className="dictation-copy">
              <span className="eyebrow">当前风格</span>
              <h2>{activeStyle?.name ?? '轻度润色'}</h2>
              <p>{formatHotkey(prefs?.hotkey ?? 'AltRight')} · {prefs?.hotkeyMode === 'toggle' ? '点击切换' : '按住说话'}</p>
            </div>
            <button className={recording ? 'record-button recording' : 'record-button'} disabled={busy && !recording} onClick={recording ? onStop : onStart}>
              {recording ? <Square size={18} /> : <Mic size={18} />}
              <span>{recording ? '停止录音' : '开始录音'}</span>
            </button>
          </section>
        </div>

        <aside className="home-side-card">
          <div className="section-title">
            <h3>本周输入</h3>
            <BarChart3 size={15} />
          </div>
          <WeekChart data={weekly} />
          <div className="week-labels">
            {weekLabels().map(label => <span key={label}>{label}</span>)}
          </div>
        </aside>
      </section>

      <section className="style-strip">
        {styles.map(style => (
          <button
            key={style.id}
            className={style.id === prefs?.activeStyleId ? 'style-chip active' : 'style-chip'}
            onClick={() => onActivateStyle(style.id)}
          >
            <SlidersHorizontal size={13} />
            {style.name}
          </button>
        ))}
      </section>

      <section className="metrics-grid">
        <Metric icon={<Hash size={14} />} label="今日字数" value={charsToday.toLocaleString()} trend={`${todayItems.length} 段输入`} />
        <Metric icon={<Mic size={14} />} label="今日次数" value={String(todayItems.length)} trend="自动保存历史" />
        <Metric icon={<Clock size={14} />} label="录音时长" value={formatDuration(durationToday)} trend="本地 ASR 处理" />
        <Metric icon={<Zap size={14} />} label="LLM" value={credentials?.llmConfigured ? '已配置' : '未配置'} trend={credentials?.llmConfigured ? '润色可用' : '需要配置'} warn={!credentials?.llmConfigured} />
      </section>

      <section className="recent-section">
        <div className="section-title">
          <h3>最近记录</h3>
          <span>{history.length} 条</span>
        </div>
        <div className="recent-list">
          {history.slice(0, 5).map(item => <RecentItem key={item.id} item={item} />)}
          {history.length === 0 && <div className="empty-state">还没有历史记录。</div>}
        </div>
      </section>
    </div>
  );
}

function ProviderCard({
  icon,
  kind,
  name,
  status,
  ready,
}: {
  icon: ReactNode;
  kind: string;
  name: string;
  status: string;
  ready: boolean;
}) {
  return (
    <div className="provider-card">
      <div className="provider-icon">{icon}</div>
      <div>
        <div className="provider-kicker">
          <span>{kind}</span>
          {ready && <CheckCircle2 size={13} />}
        </div>
        <strong>{name}</strong>
        <small>{status}</small>
      </div>
    </div>
  );
}

function Metric({ icon, label, value, trend, warn }: { icon: ReactNode; label: string; value: string; trend: string; warn?: boolean }) {
  return (
    <div className={warn ? 'metric-card warn' : 'metric-card'}>
      <span>{icon}{label}</span>
      <strong>{value}</strong>
      <small>{trend}</small>
    </div>
  );
}

function RecentItem({ item }: { item: DictationSession }) {
  return (
    <article className="recent-item">
      <div>
        <span>{formatTime(item.createdAt)}</span>
        <strong>{MODE_LABEL[item.mode]}</strong>
      </div>
      <p>{item.finalText || item.rawTranscript || '空记录'}</p>
    </article>
  );
}

function WeekChart({ data }: { data: number[] }) {
  const max = Math.max(...data, 1);
  return (
    <div className="week-chart">
      {data.map((value, index) => (
        <span key={index} className={index === data.length - 1 ? 'today' : ''}>
          <i style={{ height: `${Math.max(6, (value / max) * 92)}px` }} />
          <b>{value}</b>
        </span>
      ))}
    </div>
  );
}

function weekBuckets(history: DictationSession[]) {
  const buckets = Array(7).fill(0) as number[];
  const today = new Date();
  today.setHours(0, 0, 0, 0);
  history.forEach(item => {
    const date = new Date(item.createdAt);
    if (Number.isNaN(date.getTime())) return;
    date.setHours(0, 0, 0, 0);
    const diff = Math.floor((today.getTime() - date.getTime()) / 86400000);
    if (diff >= 0 && diff < 7) buckets[6 - diff] += 1;
  });
  return buckets;
}

function weekLabels() {
  const names = ['日', '一', '二', '三', '四', '五', '六'];
  const today = new Date().getDay();
  return Array.from({ length: 7 }, (_, index) => names[(today - 6 + index + 7) % 7]);
}
