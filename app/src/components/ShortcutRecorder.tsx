import { useEffect, useRef, useState, type KeyboardEvent } from 'react';
import { Keyboard } from 'lucide-react';
import { setShortcutRecordingActive, validateHotkey } from '../lib/ipc';

interface ShortcutRecorderProps {
  value: string;
  onChange: (value: string) => void;
}

export function ShortcutRecorder({ value, onChange }: ShortcutRecorderProps) {
  const [recording, setRecording] = useState(false);
  const [draft, setDraft] = useState('');
  const [error, setError] = useState('');
  const recorderRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (recording) recorderRef.current?.focus();
    void setShortcutRecordingActive(recording);
    return () => {
      if (recording) void setShortcutRecordingActive(false);
    };
  }, [recording]);

  useEffect(() => () => {
    void setShortcutRecordingActive(false);
  }, []);

  const start = () => {
    setRecording(true);
    setDraft('');
    setError('');
  };

  const stop = () => {
    setRecording(false);
    setDraft('');
  };

  const commit = async (binding: string) => {
    setDraft(binding);
    try {
      await validateHotkey(binding);
      onChange(binding);
      setError('');
      stop();
    } catch (err) {
      setError(String(err));
    }
  };

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (!recording) return;
    event.preventDefault();
    event.stopPropagation();
    if (event.key === 'Escape') {
      stop();
      setError('');
      return;
    }
    if (isModifierOnly(event.key)) {
      setDraft(modifiersFromEvent(event).join('+'));
      return;
    }
    const primary = primaryFromEvent(event);
    if (!primary) {
      setError('暂不支持这个按键');
      return;
    }
    const parts = [...modifiersFromEvent(event), primary];
    void commit(parts.join('+'));
  };

  return (
    <div className="shortcut-recorder">
      <button type="button" className={recording ? 'shortcut-button recording' : 'shortcut-button'} onClick={start}>
        <Keyboard size={15} />
        <span>{recording ? (draft || '按下新的快捷键') : value}</span>
      </button>
      {recording && (
        <div
          ref={recorderRef}
          tabIndex={-1}
          className="shortcut-capture"
          onKeyDown={onKeyDown}
        >
          <strong>正在录制</strong>
          <span>按下组合键保存，Esc 取消。</span>
        </div>
      )}
      {error && <div className="field-error">{error}</div>}
    </div>
  );
}

function modifiersFromEvent(event: KeyboardEvent<HTMLDivElement>) {
  const modifiers: string[] = [];
  if (event.ctrlKey && event.key !== 'Control') modifiers.push('Ctrl');
  if (event.altKey && event.key !== 'Alt') modifiers.push('Alt');
  if (event.shiftKey && event.key !== 'Shift') modifiers.push('Shift');
  if (event.metaKey && event.key !== 'Meta') modifiers.push('Win');
  return modifiers;
}

function isModifierOnly(key: string) {
  return key === 'Control' || key === 'Alt' || key === 'Shift' || key === 'Meta';
}

function primaryFromEvent(event: KeyboardEvent<HTMLDivElement>) {
  if (/^Key[A-Z]$/.test(event.code)) return event.code.slice(3);
  if (/^Digit[0-9]$/.test(event.code)) return event.code.slice(5);
  if (/^F([1-9]|1[0-2])$/.test(event.code)) return event.code;
  const codeMap: Record<string, string> = {
    Space: 'Space',
    Enter: 'Enter',
    Tab: 'Tab',
    Escape: 'Escape',
    Backspace: 'Backspace',
    Delete: 'Delete',
  };
  return codeMap[event.code] || '';
}
