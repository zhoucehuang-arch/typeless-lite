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
  const pendingModifier = useRef<string | null>(null);
  const pendingTimer = useRef<number | null>(null);

  const clearPendingModifier = () => {
    if (pendingTimer.current !== null) {
      window.clearTimeout(pendingTimer.current);
      pendingTimer.current = null;
    }
    pendingModifier.current = null;
  };

  useEffect(() => {
    if (recording) recorderRef.current?.focus();
    void setShortcutRecordingActive(recording);
    return () => {
      if (recording) void setShortcutRecordingActive(false);
    };
  }, [recording]);

  useEffect(() => () => {
    clearPendingModifier();
    void setShortcutRecordingActive(false);
  }, []);

  const start = () => {
    setRecording(true);
    setDraft('');
    setError('');
    clearPendingModifier();
  };

  const stop = () => {
    setRecording(false);
    setDraft('');
    clearPendingModifier();
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
      const primary = modifierPrimaryFromCode(event.code, event.key);
      if (!primary || pendingModifier.current === primary) return;
      clearPendingModifier();
      pendingModifier.current = primary;
      setDraft(formatHotkey(primary));
      pendingTimer.current = window.setTimeout(() => {
        if (pendingModifier.current === primary) void commit(primary);
      }, 650);
      return;
    }
    clearPendingModifier();
    const primary = primaryFromEvent(event);
    if (!primary) {
      setError('暂不支持这个按键');
      return;
    }
    const parts = [...modifiersFromEvent(event), primary];
    void commit(parts.join('+'));
  };

  const onKeyUp = (event: KeyboardEvent<HTMLDivElement>) => {
    if (!recording || !isModifierOnly(event.key)) return;
    event.preventDefault();
    event.stopPropagation();
    const primary = modifierPrimaryFromCode(event.code, event.key);
    if (primary && pendingModifier.current === primary) {
      clearPendingModifier();
      void commit(primary);
    }
  };

  return (
    <div className="shortcut-recorder">
      <button type="button" className={recording ? 'shortcut-button recording' : 'shortcut-button'} onClick={start}>
        <Keyboard size={15} />
        <span>{recording ? (draft || '按下新的快捷键') : formatHotkey(value)}</span>
      </button>
      {recording && (
        <div
          ref={recorderRef}
          tabIndex={-1}
          className="shortcut-capture"
          onKeyDown={onKeyDown}
          onKeyUp={onKeyUp}
        >
          <strong>{draft || '正在录制'}</strong>
          <span>按下快捷键保存，Esc 取消。</span>
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
  return key === 'Control' || key === 'Alt' || key === 'AltGraph' || key === 'Shift' || key === 'Meta';
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

function modifierPrimaryFromCode(code: string, key: string) {
  if (key === 'Control') return code === 'ControlRight' ? 'ControlRight' : 'ControlLeft';
  if (key === 'Alt' || key === 'AltGraph') return code === 'AltRight' ? 'AltRight' : 'AltLeft';
  if (key === 'Shift') return code === 'ShiftRight' ? 'ShiftRight' : 'ShiftLeft';
  if (key === 'Meta') return code === 'MetaRight' ? 'MetaRight' : 'MetaLeft';
  return '';
}

export function formatHotkey(binding: string) {
  return binding
    .split('+')
    .map(part => formatHotkeyPart(part.trim()))
    .filter(Boolean)
    .join('+');
}

function formatHotkeyPart(part: string) {
  const normalized = part.toLowerCase().replace(/[\s_-]/g, '');
  const labels: Record<string, string> = {
    ctrl: 'Ctrl',
    control: 'Ctrl',
    alt: 'Alt',
    option: 'Alt',
    shift: 'Shift',
    win: 'Win',
    super: 'Win',
    meta: 'Win',
    altright: '右Alt',
    rightalt: '右Alt',
    rightoption: '右Alt',
    altleft: '左Alt',
    leftalt: '左Alt',
    leftoption: '左Alt',
    controlright: '右Ctrl',
    rightcontrol: '右Ctrl',
    ctrlright: '右Ctrl',
    rightctrl: '右Ctrl',
    controlleft: '左Ctrl',
    leftcontrol: '左Ctrl',
    ctrlleft: '左Ctrl',
    leftctrl: '左Ctrl',
    shiftright: '右Shift',
    rightshift: '右Shift',
    shiftleft: '左Shift',
    leftshift: '左Shift',
    metaright: '右Win',
    rightmeta: '右Win',
    rightwin: '右Win',
    metaleft: '左Win',
    leftmeta: '左Win',
    leftwin: '左Win',
    space: 'Space',
    escape: 'Esc',
    esc: 'Esc',
  };
  if (labels[normalized]) return labels[normalized];
  if (/^key[a-z]$/.test(normalized)) return normalized.slice(3).toUpperCase();
  if (/^digit[0-9]$/.test(normalized)) return normalized.slice(5);
  if (part.length === 1 && /[a-z]/i.test(part)) return part.toUpperCase();
  return part;
}
