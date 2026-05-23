import { useCallback, useEffect, useMemo, useState } from 'react';
import { BookOpen, History as HistoryIcon, Home as HomeIcon, Settings, SlidersHorizontal } from 'lucide-react';
import { Capsule } from './components/Capsule';
import { SettingsModal } from './components/SettingsModal';
import {
  addCorrectionRule,
  addDictionaryEntry,
  appStatus,
  clearHistory,
  deleteHistoryEntry,
  getCredentials,
  getSettings,
  listCorrectionRules,
  listDictionary,
  listHistory,
  listStyles,
  removeCorrectionRule,
  removeDictionaryEntry,
  resetBuiltinStyle,
  saveStyle,
  setCorrectionRuleEnabled,
  setDictionaryEntryEnabled,
  setSettings,
  startDictation,
  stopDictation,
} from './lib/ipc';
import type {
  AppStatus,
  CorrectionRule,
  CredentialsStatus,
  DictationSession,
  DictionaryEntry,
  Preferences,
  StyleProfile,
} from './lib/types';
import { Home } from './pages/Home';
import { History } from './pages/History';
import { Dictionary } from './pages/Dictionary';
import { Styles } from './pages/Styles';

type Tab = 'home' | 'history' | 'dictionary' | 'styles';

const NAV = [
  { id: 'home', label: '首页', icon: HomeIcon },
  { id: 'history', label: '历史', icon: HistoryIcon },
  { id: 'dictionary', label: '词典', icon: BookOpen },
  { id: 'styles', label: '风格', icon: SlidersHorizontal },
] as const;

export function App() {
  const [tab, setTab] = useState<Tab>('home');
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [prefs, setPrefs] = useState<Preferences | null>(null);
  const [history, setHistory] = useState<DictationSession[]>([]);
  const [dictionary, setDictionary] = useState<DictionaryEntry[]>([]);
  const [rules, setRules] = useState<CorrectionRule[]>([]);
  const [styles, setStyles] = useState<StyleProfile[]>([]);
  const [credentials, setCredentials] = useState<CredentialsStatus | null>(null);
  const [status, setStatus] = useState<AppStatus | null>(null);
  const [recording, setRecording] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refreshHistory = useCallback(() => {
    void listHistory().then(setHistory).catch(err => setError(String(err)));
  }, []);

  const refreshDictionary = useCallback(() => {
    void Promise.all([listDictionary(), listCorrectionRules()])
      .then(([entries, nextRules]) => {
        setDictionary(entries);
        setRules(nextRules);
      })
      .catch(err => setError(String(err)));
  }, []);

  const refreshStyles = useCallback(() => {
    void listStyles().then(setStyles).catch(err => setError(String(err)));
  }, []);

  const refreshAll = useCallback(() => {
    void appStatus().then(setStatus).catch(() => undefined);
    void getSettings().then(setPrefs).catch(err => setError(String(err)));
    void getCredentials().then(setCredentials).catch(() => setCredentials(null));
    refreshHistory();
    refreshDictionary();
    refreshStyles();
  }, [refreshDictionary, refreshHistory, refreshStyles]);

  useEffect(() => {
    refreshAll();
  }, [refreshAll]);

  const activePage = useMemo(() => {
    if (tab === 'history') {
      return (
        <History
          items={history}
          onRefresh={refreshHistory}
          onDelete={id => {
            void deleteHistoryEntry(id).then(refreshHistory).catch(err => setError(String(err)));
          }}
          onClear={() => {
            void clearHistory().then(refreshHistory).catch(err => setError(String(err)));
          }}
        />
      );
    }
    if (tab === 'dictionary') {
      return (
        <Dictionary
          entries={dictionary}
          rules={rules}
          onRefresh={refreshDictionary}
          onAddEntry={(phrase, note) => {
            void addDictionaryEntry(phrase, note).then(refreshDictionary).catch(err => setError(String(err)));
          }}
          onRemoveEntry={id => {
            void removeDictionaryEntry(id).then(refreshDictionary).catch(err => setError(String(err)));
          }}
          onToggleEntry={(id, enabled) => {
            void setDictionaryEntryEnabled(id, enabled).then(refreshDictionary).catch(err => setError(String(err)));
          }}
          onAddRule={(pattern, replacement) => {
            void addCorrectionRule(pattern, replacement).then(refreshDictionary).catch(err => setError(String(err)));
          }}
          onRemoveRule={id => {
            void removeCorrectionRule(id).then(refreshDictionary).catch(err => setError(String(err)));
          }}
          onToggleRule={(id, enabled) => {
            void setCorrectionRuleEnabled(id, enabled).then(refreshDictionary).catch(err => setError(String(err)));
          }}
        />
      );
    }
    if (tab === 'styles') {
      return (
        <Styles
          prefs={prefs}
          styles={styles}
          onActivate={activateStyle}
          onSave={style => {
            void saveStyle(style).then(refreshStyles).catch(err => setError(String(err)));
          }}
          onReset={id => {
            void resetBuiltinStyle(id).then(refreshStyles).catch(err => setError(String(err)));
          }}
        />
      );
    }
    return (
      <Home
        prefs={prefs}
        styles={styles}
        history={history}
        credentials={credentials}
        recording={recording}
        busy={busy}
        onStart={beginRecording}
        onStop={finishRecording}
        onOpenSettings={() => setSettingsOpen(true)}
        onActivateStyle={activateStyle}
      />
    );
  }, [busy, credentials, dictionary, history, prefs, recording, refreshDictionary, refreshHistory, refreshStyles, rules, styles, tab]);

  async function beginRecording() {
    setBusy(true);
    setError(null);
    try {
      await startDictation();
      setRecording(true);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  async function finishRecording() {
    setBusy(true);
    setError(null);
    try {
      await stopDictation();
      setRecording(false);
      refreshHistory();
      refreshDictionary();
    } catch (err) {
      setRecording(false);
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  function activateStyle(id: string) {
    if (!prefs) return;
    const next = { ...prefs, activeStyleId: id };
    setPrefs(next);
    void setSettings(next).catch(err => setError(String(err)));
  }

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <div className="brand-mark">T</div>
          <div>
            <strong>Typeless Lite</strong>
            <span>{status?.platform ?? 'desktop'} · v{status?.version ?? '0.1.0'}</span>
          </div>
        </div>
        <nav>
          {NAV.map(item => {
            const Icon = item.icon;
            return (
              <button key={item.id} className={tab === item.id ? 'nav-item active' : 'nav-item'} onClick={() => setTab(item.id)}>
                <Icon size={16} />
                {item.label}
              </button>
            );
          })}
        </nav>
        <button className={settingsOpen ? 'nav-item active settings-entry' : 'nav-item settings-entry'} onClick={() => setSettingsOpen(true)}>
          <Settings size={16} />
          设置
        </button>
      </aside>

      <main className="content">
        {error && (
          <div className="error-banner">
            <span>{error}</span>
            <button onClick={() => setError(null)}>关闭</button>
          </div>
        )}
        {activePage}
      </main>

      <Capsule />

      {settingsOpen && prefs && (
        <SettingsModal
          prefs={prefs}
          onClose={() => setSettingsOpen(false)}
          onSaved={next => {
            setPrefs(next);
            void getCredentials().then(setCredentials);
          }}
        />
      )}
    </div>
  );
}
