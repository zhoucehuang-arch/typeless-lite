import { useEffect, useMemo, useState, type ReactNode } from 'react';
import { CheckCircle2, Cpu, Database, Download, Eraser, FolderOpen, Mic, RefreshCw, ShieldCheck, Sparkles, X } from 'lucide-react';
import {
  clearLocalData,
  getCredentials,
  listLlmModels,
  listMicrophones,
  localDataStatus,
  onSherpaDownloadProgress,
  setLlmApiKey,
  setSettings,
  sherpaDefaultModelStatus,
  sherpaPrepareDefaultModel,
  validateLlmModel,
} from '../lib/ipc';
import type {
  CredentialsStatus,
  LocalDataStatus,
  MicrophoneDevice,
  Preferences,
  SherpaDefaultModelStatus,
  SherpaDownloadProgress,
} from '../lib/types';
import { ShortcutRecorder } from './ShortcutRecorder';

interface SettingsModalProps {
  prefs: Preferences;
  onClose: () => void;
  onSaved: (prefs: Preferences) => void;
}

export function SettingsModal({ prefs, onClose, onSaved }: SettingsModalProps) {
  const [section, setSection] = useState<'dictation' | 'services' | 'output' | 'data'>('dictation');
  const [draft, setDraft] = useState(prefs);
  const [apiKey, setApiKey] = useState('');
  const [credentials, setCredentials] = useState<CredentialsStatus | null>(null);
  const [microphones, setMicrophones] = useState<MicrophoneDevice[]>([]);
  const [asrStatus, setAsrStatus] = useState<SherpaDefaultModelStatus | null>(null);
  const [downloadProgress, setDownloadProgress] = useState<SherpaDownloadProgress | null>(null);
  const [dataStatus, setDataStatus] = useState<LocalDataStatus | null>(null);
  const [llmModels, setLlmModels] = useState<string[]>([]);
  const [loadingModels, setLoadingModels] = useState(false);
  const [checkingModel, setCheckingModel] = useState(false);
  const [saving, setSaving] = useState(false);
  const [status, setStatus] = useState('');

  useEffect(() => {
    setDraft(prefs);
  }, [prefs]);

  useEffect(() => {
    void getCredentials().then(setCredentials).catch(() => setCredentials(null));
    void listMicrophones().then(setMicrophones).catch(() => setMicrophones([]));
    void sherpaDefaultModelStatus().then(setAsrStatus).catch(error => setStatus(String(error)));
    void localDataStatus().then(setDataStatus).catch(() => setDataStatus(null));
    const unlisten = onSherpaDownloadProgress(payload => {
      setDownloadProgress(payload);
      if (payload.done) void sherpaDefaultModelStatus().then(setAsrStatus);
    });
    return () => {
      void unlisten.then(dispose => dispose());
    };
  }, []);

  const progressLabel = useMemo(() => {
    if (!downloadProgress || downloadProgress.totalBytes <= 0) return '';
    const percent = Math.round((downloadProgress.downloadedBytes / downloadProgress.totalBytes) * 100);
    return `${percent}% · ${formatBytes(downloadProgress.downloadedBytes)} / ${formatBytes(downloadProgress.totalBytes)}`;
  }, [downloadProgress]);

  const save = async () => {
    setSaving(true);
    setStatus('');
    try {
      if (apiKey.trim()) {
        await setLlmApiKey(apiKey.trim());
        setApiKey('');
      }
      const normalized = {
        ...draft,
        asrProvider: 'sherpa-onnx-local',
        sherpaModel: 'sense-voice-small-zh',
        sherpaLanguageHint: draft.sherpaLanguageHint || 'zh',
      };
      await setSettings(normalized);
      onSaved(normalized);
      setStatus('已保存');
      void getCredentials().then(setCredentials);
      void localDataStatus().then(setDataStatus);
    } catch (err) {
      setStatus(String(err));
    } finally {
      setSaving(false);
    }
  };

  const prepareAsr = async () => {
    setStatus('');
    setDownloadProgress(null);
    try {
      const next = await sherpaPrepareDefaultModel();
      setAsrStatus(next);
      setStatus(next.cached ? '本地 ASR 模型已缓存' : '模型准备完成');
    } catch (err) {
      setStatus(String(err));
    }
  };

  const fetchModels = async () => {
    setLoadingModels(true);
    setStatus('');
    try {
      const models = await listLlmModels(draft.llmBaseUrl, apiKey || null);
      setLlmModels(models);
      if (models.length > 0 && !models.includes(draft.llmModel)) {
        const preferred = pickPreferredModel(models);
        setDraft(current => ({ ...current, llmModel: preferred }));
      }
      setStatus(models.length > 0 ? `已获取 ${models.length} 个模型` : '没有返回可用模型');
    } catch (err) {
      setStatus(String(err));
    } finally {
      setLoadingModels(false);
    }
  };

  const checkModel = async () => {
    setCheckingModel(true);
    setStatus('');
    try {
      const result = await validateLlmModel(draft.llmBaseUrl, draft.llmModel, apiKey || null);
      setStatus(result.message);
    } catch (err) {
      setStatus(String(err));
    } finally {
      setCheckingModel(false);
    }
  };

  const clearData = async (options: Parameters<typeof clearLocalData>[0], message: string) => {
    if (!window.confirm(message)) return;
    setStatus('');
    try {
      const next = await clearLocalData(options);
      setDataStatus(next);
      void getCredentials().then(setCredentials);
      setStatus('本地数据已更新');
    } catch (err) {
      setStatus(String(err));
    }
  };

  return (
    <div className="modal-backdrop" onMouseDown={onClose}>
      <section className="settings-modal" onMouseDown={event => event.stopPropagation()}>
        <aside className="settings-sidebar">
          <div className="settings-brand">
            <strong>设置</strong>
            <button type="button" className="icon-button" title="关闭" onClick={onClose}>
              <X size={15} />
            </button>
          </div>
          <button type="button" className={section === 'dictation' ? 'settings-tab active' : 'settings-tab'} onClick={() => setSection('dictation')}>
            <Mic size={14} />
            听写
          </button>
          <button type="button" className={section === 'services' ? 'settings-tab active' : 'settings-tab'} onClick={() => setSection('services')}>
            <Cpu size={14} />
            模型
          </button>
          <button type="button" className={section === 'output' ? 'settings-tab active' : 'settings-tab'} onClick={() => setSection('output')}>
            <Sparkles size={14} />
            输出
          </button>
          <button type="button" className={section === 'data' ? 'settings-tab active' : 'settings-tab'} onClick={() => setSection('data')}>
            <Database size={14} />
            数据
          </button>
        </aside>

        <div className="settings-main">
          <div className="settings-scroll">
            {section === 'dictation' && (
              <SettingsPanel title="听写">
                <SettingRow label="快捷键">
                  <ShortcutRecorder value={draft.hotkey} onChange={hotkey => setDraft({ ...draft, hotkey })} />
                </SettingRow>
                <SettingRow label="录音模式">
                  <select value={draft.hotkeyMode} onChange={event => setDraft({ ...draft, hotkeyMode: event.target.value as Preferences['hotkeyMode'] })}>
                    <option value="hold">按住说话</option>
                    <option value="toggle">点击切换</option>
                  </select>
                </SettingRow>
                <SettingRow label="麦克风">
                  <select value={draft.microphoneDeviceName ?? ''} onChange={event => setDraft({ ...draft, microphoneDeviceName: event.target.value || null })}>
                    <option value="">系统默认</option>
                    {microphones.map(device => (
                      <option key={device.name} value={device.name}>{device.name}{device.isDefault ? '（默认）' : ''}</option>
                    ))}
                  </select>
                </SettingRow>
                <SettingRow label="状态胶囊">
                  <Toggle checked={draft.showCapsule} onChange={checked => setDraft({ ...draft, showCapsule: checked })} />
                </SettingRow>
                <SettingRow label="恢复剪贴板">
                  <Toggle checked={draft.restoreClipboardAfterPaste} onChange={checked => setDraft({ ...draft, restoreClipboardAfterPaste: checked })} />
                </SettingRow>
              </SettingsPanel>
            )}

            {section === 'services' && (
              <SettingsPanel title="模型">
                <SettingRow label="本地 ASR">
                  <div className={asrStatus?.cached ? 'model-status ready' : 'model-status'}>
                    <div>
                      <strong>{asrStatus?.displayName ?? 'SenseVoice Small'}</strong>
                      <span>sherpa-onnx-local · sense-voice-small-zh</span>
                    </div>
                    {asrStatus?.cached ? <CheckCircle2 size={18} /> : <Download size={18} />}
                  </div>
                </SettingRow>
                <SettingRow label="模型缓存">
                  <div className="stack-control">
                    <button type="button" className="tool-button" onClick={() => void prepareAsr()}>
                      <Download size={15} />
                      {asrStatus?.cached ? '重新检查缓存' : '缓存默认模型'}
                    </button>
                    {downloadProgress && !downloadProgress.done && (
                      <div className="progress-line">
                        <span style={{ width: downloadProgress.totalBytes > 0 ? `${Math.min(100, (downloadProgress.downloadedBytes / downloadProgress.totalBytes) * 100)}%` : '20%' }} />
                      </div>
                    )}
                    <div className="hint-line">{progressLabel || asrStatus?.directory || '模型目录未就绪'}</div>
                  </div>
                </SettingRow>
                <SettingRow label="Base URL">
                  <input value={draft.llmBaseUrl} onChange={event => setDraft({ ...draft, llmBaseUrl: event.target.value })} />
                </SettingRow>
                <SettingRow label="API Key">
                  <input type="password" value={apiKey} placeholder={credentials?.llmConfigured ? '已配置，填写可覆盖' : '未配置'} onChange={event => setApiKey(event.target.value)} />
                </SettingRow>
                <SettingRow label="模型">
                  {llmModels.length > 0 ? (
                    <select value={draft.llmModel} onChange={event => setDraft({ ...draft, llmModel: event.target.value })}>
                      {llmModels.map(model => (
                        <option key={model} value={model}>{model}</option>
                      ))}
                    </select>
                  ) : (
                    <input value={draft.llmModel} onChange={event => setDraft({ ...draft, llmModel: event.target.value })} />
                  )}
                </SettingRow>
                <SettingRow label="工具">
                  <div className="tool-row">
                    <button type="button" className="tool-button" disabled={loadingModels} onClick={() => void fetchModels()}>
                      <RefreshCw size={15} className={loadingModels ? 'spin' : ''} />
                      获取模型列表
                    </button>
                    <button type="button" className="tool-button" disabled={checkingModel} onClick={() => void checkModel()}>
                      <ShieldCheck size={15} />
                      检验模型可用
                    </button>
                  </div>
                </SettingRow>
              </SettingsPanel>
            )}

            {section === 'output' && (
              <SettingsPanel title="输出">
                <SettingRow label="输出语言">
                  <select value={draft.outputLanguage} onChange={event => setDraft({ ...draft, outputLanguage: event.target.value as Preferences['outputLanguage'] })}>
                    <option value="auto">跟随原文</option>
                    <option value="zhCn">简体中文</option>
                    <option value="en">英文</option>
                  </select>
                </SettingRow>
                <SettingRow label="历史上限">
                  <input type="number" min={5} max={200} value={draft.historyMaxEntries} onChange={event => setDraft({ ...draft, historyMaxEntries: Number(event.target.value) })} />
                </SettingRow>
              </SettingsPanel>
            )}

            {section === 'data' && (
              <SettingsPanel title="数据">
                <SettingRow label="凭据状态">
                  <div className={dataStatus?.llmApiKeyFoundInJson ? 'model-status danger' : 'model-status ready'}>
                    <div>
                      <strong>{dataStatus?.llmApiKeyFoundInJson ? '发现疑似明文凭据' : 'JSON 文件未发现 API key'}</strong>
                      <span>{dataStatus?.dataDir ?? '数据目录未就绪'}</span>
                    </div>
                    {dataStatus?.llmApiKeyFoundInJson ? <ShieldCheck size={18} /> : <Database size={18} />}
                  </div>
                </SettingRow>
                <SettingRow label="本地文件">
                  <div className="data-file-grid">
                    {dataStatus?.files.map(file => (
                      <div key={file.name} className="data-file-row">
                        <strong>{file.name}</strong>
                        <span>{file.exists ? `${formatBytes(file.bytes)}${file.records === null ? '' : ` · ${file.records} 条`}` : '未创建'}</span>
                      </div>
                    ))}
                  </div>
                </SettingRow>
                <SettingRow label="工具">
                  <div className="tool-row">
                    <button type="button" className="tool-button" onClick={() => void localDataStatus().then(setDataStatus)}>
                      <RefreshCw size={15} />
                      刷新状态
                    </button>
                    <button type="button" className="tool-button" onClick={() => navigator.clipboard.writeText(dataStatus?.dataDir ?? '')}>
                      <FolderOpen size={15} />
                      复制数据目录
                    </button>
                    <button type="button" className="tool-button danger" onClick={() => void clearData({ history: true }, '确定清空历史记录？')}>
                      <Eraser size={15} />
                      清空历史
                    </button>
                    <button type="button" className="tool-button danger" onClick={() => void clearData({ dictionary: true }, '确定清空词典和纠错规则？')}>
                      <Eraser size={15} />
                      清空词典
                    </button>
                    <button type="button" className="tool-button danger" onClick={() => void clearData({ styles: true }, '确定重置所有润色风格？')}>
                      <Eraser size={15} />
                      重置风格
                    </button>
                    <button type="button" className="tool-button danger" onClick={() => void clearData({ apiKey: true }, '确定删除系统凭据库中的 LLM API Key？')}>
                      <Eraser size={15} />
                      删除 API Key
                    </button>
                    <button type="button" className="tool-button danger" onClick={() => void clearData({ settings: true }, '确定重置偏好设置？快捷键等配置会恢复默认值。')}>
                      <Eraser size={15} />
                      重置设置
                    </button>
                  </div>
                </SettingRow>
              </SettingsPanel>
            )}
          </div>

          <footer>
            <span>{status}</span>
            <button className="primary" disabled={saving} onClick={() => void save()}>{saving ? '保存中' : '保存设置'}</button>
          </footer>
        </div>
      </section>
    </div>
  );
}

function pickPreferredModel(models: string[]) {
  const preferred = [
    'gpt-5.2-chat-latest',
    'gpt-5.2',
    'gpt-5.4',
    'gpt-5.5',
    'gpt-5.4-mini',
    'deepseek-chat',
  ];
  const lower = new Map(models.map(model => [model.toLowerCase(), model]));
  for (const model of preferred) {
    const match = lower.get(model);
    if (match) return match;
  }
  return models[0];
}

function SettingsPanel({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="settings-panel">
      <h2>{title}</h2>
      <div>{children}</div>
    </section>
  );
}

function SettingRow({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="setting-row">
      <div className="setting-label">{label}</div>
      <div className="setting-control">{children}</div>
    </div>
  );
}

function Toggle({ checked, onChange }: { checked: boolean; onChange: (checked: boolean) => void }) {
  return (
    <button type="button" className={checked ? 'toggle active' : 'toggle'} onClick={() => onChange(!checked)}>
      <span />
    </button>
  );
}

function formatBytes(bytes: number) {
  if (bytes <= 0) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB'];
  let size = bytes;
  let unit = 0;
  while (size >= 1024 && unit < units.length - 1) {
    size /= 1024;
    unit += 1;
  }
  return `${size.toFixed(unit === 0 ? 0 : 1)} ${units[unit]}`;
}
