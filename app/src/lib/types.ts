export type PolishMode = 'raw' | 'light' | 'structured' | 'formal';
export type HotkeyMode = 'hold' | 'toggle';
export type OutputLanguage = 'auto' | 'zhCn' | 'en';
export type InsertStatus = 'inserted' | 'copiedFallback' | 'failed';

export interface Preferences {
  hotkey: string;
  hotkeyMode: HotkeyMode;
  launchAtLogin: boolean;
  showCapsule: boolean;
  microphoneDeviceName: string | null;
  activeStyleId: string;
  outputLanguage: OutputLanguage;
  asrProvider: string;
  sherpaModel: string;
  sherpaLanguageHint: string | null;
  sherpaKeepLoadedSecs: number;
  llmBaseUrl: string;
  llmModel: string;
  restoreClipboardAfterPaste: boolean;
  historyMaxEntries: number;
}

export interface DictationSession {
  id: string;
  createdAt: string;
  rawTranscript: string;
  finalText: string;
  mode: PolishMode;
  insertStatus: InsertStatus;
  errorCode: string | null;
  durationMs: number;
  dictionaryHitCount: number;
}

export interface DictionaryEntry {
  id: string;
  phrase: string;
  note: string | null;
  enabled: boolean;
  hits: number;
  createdAt: string;
}

export interface CorrectionRule {
  id: string;
  pattern: string;
  replacement: string;
  enabled: boolean;
  createdAt: string;
}

export interface StyleProfile {
  id: string;
  name: string;
  mode: PolishMode;
  prompt: string;
  builtin: boolean;
  updatedAt: string;
}

export interface CredentialsStatus {
  llmConfigured: boolean;
}

export interface MicrophoneDevice {
  name: string;
  isDefault: boolean;
}

export interface SherpaModelInfo {
  alias: string;
  displayName: string;
  languages: string[];
  cached: boolean;
}

export interface SherpaModelFileStatus {
  name: string;
  present: boolean;
  bytes: number;
}

export interface SherpaDefaultModelStatus {
  alias: string;
  displayName: string;
  cached: boolean;
  directory: string;
  files: SherpaModelFileStatus[];
  downloadedBytes: number;
}

export interface SherpaDownloadProgress {
  alias: string;
  file: string;
  downloadedBytes: number;
  totalBytes: number;
  done: boolean;
  error: string | null;
}

export interface LlmValidationResult {
  ok: boolean;
  message: string;
}

export interface CapsulePayload {
  state: 'idle' | 'recording' | 'transcribing' | 'polishing' | 'done' | 'cancelled' | 'error';
  level: number;
  elapsedMs: number;
  message: string | null;
  insertedChars: number | null;
}

export interface AppStatus {
  version: string;
  platform: string;
}

export const MODE_LABEL: Record<PolishMode, string> = {
  raw: '原文',
  light: '轻度润色',
  structured: '清晰结构',
  formal: '正式表达',
};
