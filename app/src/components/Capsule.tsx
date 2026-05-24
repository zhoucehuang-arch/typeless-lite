import { useEffect, useState } from 'react';
import { Check, Loader2, X } from 'lucide-react';
import { cancelDictation, onCapsule, stopDictation } from '../lib/ipc';
import type { CapsulePayload } from '../lib/types';

const initial: CapsulePayload = {
  state: 'idle',
  level: 0,
  elapsedMs: 0,
  message: null,
  insertedChars: null,
};

export function Capsule() {
  const [payload, setPayload] = useState<CapsulePayload>(initial);
  const [leaving, setLeaving] = useState(false);
  const [visiblePayload, setVisiblePayload] = useState<CapsulePayload>(initial);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let leaveTimer: number | undefined;
    void onCapsule(next => {
      setPayload(next);
      if (next.state !== 'idle') {
        if (leaveTimer) window.clearTimeout(leaveTimer);
        setLeaving(false);
        setVisiblePayload(next);
      } else {
        setLeaving(true);
        leaveTimer = window.setTimeout(() => setLeaving(false), 360);
      }
    }).then(fn => {
      unlisten = fn;
    });
    return () => {
      if (leaveTimer) window.clearTimeout(leaveTimer);
      unlisten?.();
    };
  }, []);

  const active = payload.state !== 'idle' || leaving;
  const rendered = payload.state === 'idle' ? visiblePayload : payload;
  const recording = rendered.state === 'recording';

  if (!active) {
    return <div className="capsule-root" />;
  }

  return (
    <div className="capsule-root">
      <div className={leaving ? `capsule ${rendered.state} leaving` : `capsule ${rendered.state}`}>
        <button className="capsule-action secondary" disabled={!recording} aria-label="取消录音" onClick={() => void cancelDictation()}>
          <X size={13} />
        </button>

        <div className="capsule-center">
          {recording ? (
            <Waveform level={rendered.level} />
          ) : (
            <div className="capsule-status">
              {(rendered.state === 'transcribing' || rendered.state === 'polishing') && <Loader2 size={16} className="spin" />}
              {rendered.state === 'done' && <Check size={16} />}
              {(rendered.state === 'error' || rendered.state === 'cancelled') && <X size={16} />}
              <span>{labelFor(rendered)}</span>
            </div>
          )}
          {rendered.message && <small>{rendered.message}</small>}
        </div>

        <button className="capsule-action primary-action" disabled={!recording} aria-label="结束录音" onClick={() => void stopDictation()}>
          <Check size={14} />
        </button>
      </div>
    </div>
  );
}

function Waveform({ level }: { level: number }) {
  const envelope = [0.35, 0.58, 0.82, 1, 0.86, 0.62, 0.42];
  const voice = Math.min(1, Math.max(0, level));
  const gated = Math.min(1, Math.max(0, (voice - 0.02) / 0.32));
  const visual = Math.pow(gated * gated * (3 - 2 * gated), 0.42);

  return (
    <div className="capsule-wave" aria-label="录音音量">
      {envelope.map((item, index) => {
        const height = 4 + 24 * Math.max(0.08, visual * item);
        return (
          <span
            key={index}
            style={{
              height: `${height}px`,
              animationDelay: `${index * 45}ms`,
            }}
          />
        );
      })}
    </div>
  );
}

function labelFor(payload: CapsulePayload) {
  switch (payload.state) {
    case 'recording':
      return '正在录音';
    case 'transcribing':
      return '正在识别';
    case 'polishing':
      return '正在润色';
    case 'done':
      return payload.insertedChars ? `已插入 ${payload.insertedChars} 字` : '已完成';
    case 'cancelled':
      return '已取消';
    case 'error':
      return '出现错误';
    default:
      return '待机';
  }
}
