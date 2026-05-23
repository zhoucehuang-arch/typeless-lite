import { useEffect, useState } from 'react';
import { X } from 'lucide-react';
import {
  getCredentials,
  listMicrophones,
  setLlmApiKey,
  setSettings,
  sherpaCatalog,
  sherpaModelDir,
} from '../lib/ipc';
import type { CredentialsStatus, MicrophoneDevice, Preferences, SherpaModelInfo } from '../lib/types';
import { IconButton } from './IconButton';

interface SettingsModalProps {
  prefs: Preferences;
  onClose: () => void;
  onSaved: (prefs: Preferences) => void;
}

export function SettingsModal({ prefs, onClose, onSaved }: SettingsModalProps) {
  const [draft, setDraft] = useState(prefs);
  const [apiKey, setApiKey] = useState('');
  const [credentials, setCredentials] = useState<CredentialsStatus | null>(null);
  const [microphones, setMicrophones] = useState<MicrophoneDevice[]>([]);
  const [models, setModels] = useState<SherpaModelInfo[]>([]);
  const [modelDir, setModelDir] = useState('');
  const [status, setStatus] = useState('');

  useEffect(() => {
    setDraft(prefs);
  }, [prefs]);

  useEffect(() => {
    void getCredentials().then(setCredentials).catch(() => setCredentials(null));
    void listMicrophones().then(setMicrophones).catch(() => setMicrophones([]));
    void sherpaCatalog().then(setModels).catch(() => setModels([]));
  }, []);

  useEffect(() => {
    void sherpaModelDir(draft.sherpaModel).then(setModelDir).catch(error => setModelDir(String(error)));
  }, [draft.sherpaModel]);

  const save = async () => {
    await setSettings(draft);
    if (apiKey.trim()) {
      await setLlmApiKey(apiKey.trim());
      setApiKey('');
    }
    onSaved(draft);
    setStatus('已保存');
    window.setTimeout(() => setStatus(''), 1200);
  };

  return (
    <div className="modal-backdrop" onMouseDown={onClose}>
      <section className="settings-modal" onMouseDown={event => event.stopPropagation()}>
        <header>
          <div>
            <h2>设置</h2>
            <p>调整听写、模型、润色和本地数据。</p>
          </div>
          <IconButton title="关闭" onClick={onClose}>
            <X size={16} />
          </IconButton>
        </header>

        <div className="settings-grid">
          <section className="settings-section">
            <h3>通用</h3>
            <label>
              快捷键
              <input value={draft.hotkey} onChange={event => setDraft({ ...draft, hotkey: event.target.value })} />
            </label>
            <label>
              录音模式
              <select value={draft.hotkeyMode} onChange={event => setDraft({ ...draft, hotkeyMode: event.target.value as Preferences['hotkeyMode'] })}>
                <option value="hold">按住说话</option>
                <option value="toggle">点击切换</option>
              </select>
            </label>
            <label className="check-row">
              <input type="checkbox" checked={draft.showCapsule} onChange={event => setDraft({ ...draft, showCapsule: event.target.checked })} />
              显示状态胶囊
            </label>
            <label className="check-row">
              <input type="checkbox" checked={draft.restoreClipboardAfterPaste} onChange={event => setDraft({ ...draft, restoreClipboardAfterPaste: event.target.checked })} />
              粘贴后恢复剪贴板
            </label>
          </section>

          <section className="settings-section">
            <h3>音频与 ASR</h3>
            <label>
              麦克风
              <select value={draft.microphoneDeviceName ?? ''} onChange={event => setDraft({ ...draft, microphoneDeviceName: event.target.value || null })}>
                <option value="">系统默认</option>
                {microphones.map(device => (
                  <option key={device.name} value={device.name}>{device.name}{device.isDefault ? '（默认）' : ''}</option>
                ))}
              </select>
            </label>
            <label>
              本地模型
              <select value={draft.sherpaModel} onChange={event => setDraft({ ...draft, sherpaModel: event.target.value })}>
                {models.map(model => (
                  <option key={model.alias} value={model.alias}>{model.displayName}{model.cached ? '' : '（未缓存）'}</option>
                ))}
              </select>
            </label>
            <label>
              语言 hint
              <input value={draft.sherpaLanguageHint ?? ''} onChange={event => setDraft({ ...draft, sherpaLanguageHint: event.target.value || null })} />
            </label>
            <div className="path-box">{modelDir || '模型目录未就绪'}</div>
          </section>

          <section className="settings-section">
            <h3>LLM</h3>
            <label>
              Base URL
              <input value={draft.llmBaseUrl} onChange={event => setDraft({ ...draft, llmBaseUrl: event.target.value })} />
            </label>
            <label>
              模型
              <input value={draft.llmModel} onChange={event => setDraft({ ...draft, llmModel: event.target.value })} />
            </label>
            <label>
              Temperature
              <input type="number" min={0} max={2} step={0.1} value={draft.llmTemperature} onChange={event => setDraft({ ...draft, llmTemperature: Number(event.target.value) })} />
            </label>
            <label>
              API Key
              <input type="password" value={apiKey} placeholder={credentials?.llmConfigured ? '已配置，填写可覆盖' : '未配置'} onChange={event => setApiKey(event.target.value)} />
            </label>
          </section>

          <section className="settings-section">
            <h3>数据</h3>
            <label>
              历史上限
              <input type="number" min={5} max={200} value={draft.historyMaxEntries} onChange={event => setDraft({ ...draft, historyMaxEntries: Number(event.target.value) })} />
            </label>
            <label>
              输出语言
              <select value={draft.outputLanguage} onChange={event => setDraft({ ...draft, outputLanguage: event.target.value as Preferences['outputLanguage'] })}>
                <option value="auto">跟随原文</option>
                <option value="zhCn">简体中文</option>
                <option value="en">英文</option>
              </select>
            </label>
          </section>
        </div>

        <footer>
          <span>{status}</span>
          <button className="primary" onClick={() => void save()}>保存设置</button>
        </footer>
      </section>
    </div>
  );
}
