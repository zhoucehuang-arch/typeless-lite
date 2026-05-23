import { useEffect, useState } from 'react';
import { Check, Loader2, Mic, XCircle } from 'lucide-react';
import { onCapsule } from '../lib/ipc';
import type { CapsulePayload } from '../lib/types';
import { formatDuration } from '../lib/format';

const initial: CapsulePayload = {
  state: 'idle',
  level: 0,
  elapsedMs: 0,
  message: null,
  insertedChars: null,
};

export function Capsule() {
  const [payload, setPayload] = useState<CapsulePayload>(initial);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void onCapsule(next => setPayload(next)).then(fn => {
      unlisten = fn;
    });
    return () => unlisten?.();
  }, []);

  const active = payload.state !== 'idle';
  return (
    <div className="capsule-root">
      <div className={active ? `capsule ${payload.state}` : 'capsule idle'}>
        <div className="capsule-icon">
          {payload.state === 'recording' && <Mic size={15} />}
          {(payload.state === 'transcribing' || payload.state === 'polishing') && <Loader2 size={15} className="spin" />}
          {payload.state === 'done' && <Check size={15} />}
          {(payload.state === 'error' || payload.state === 'cancelled') && <XCircle size={15} />}
          {payload.state === 'idle' && <Mic size={15} />}
        </div>
        <div className="capsule-main">
          <span>{labelFor(payload)}</span>
          <small>{payload.message ?? formatDuration(payload.elapsedMs)}</small>
        </div>
        {payload.state === 'recording' && (
          <div className="capsule-meter">
            <span style={{ transform: `scaleY(${Math.max(0.1, payload.level)})` }} />
          </div>
        )}
      </div>
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
