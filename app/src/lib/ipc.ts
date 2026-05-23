import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type {
  AppStatus,
  CapsulePayload,
  ClearLocalDataOptions,
  CorrectionRule,
  CredentialsStatus,
  DictationSession,
  DictionaryEntry,
  LocalDataStatus,
  MicrophoneDevice,
  Preferences,
  LlmValidationResult,
  SherpaDefaultModelStatus,
  SherpaDownloadProgress,
  SherpaModelInfo,
  StyleProfile,
} from './types';

export const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

export function appStatus() {
  return invoke<AppStatus>('app_status');
}

export function getSettings() {
  return invoke<Preferences>('get_settings');
}

export function setSettings(prefs: Preferences) {
  return invoke<void>('set_settings', { prefs });
}

export function getCredentials() {
  return invoke<CredentialsStatus>('get_credentials');
}

export function setLlmApiKey(apiKey: string) {
  return invoke<void>('set_llm_api_key', { apiKey });
}

export function localDataStatus() {
  return invoke<LocalDataStatus>('local_data_status');
}

export function clearLocalData(options: ClearLocalDataOptions) {
  return invoke<LocalDataStatus>('clear_local_data', { options });
}

export function validateHotkey(binding: string) {
  return invoke<void>('validate_hotkey', { binding });
}

export function setShortcutRecordingActive(active: boolean) {
  return invoke<void>('set_shortcut_recording_active', { active });
}

export function listLlmModels(baseUrl: string, apiKey?: string | null) {
  return invoke<string[]>('list_llm_models', { baseUrl, apiKey: apiKey || null });
}

export function validateLlmModel(baseUrl: string, model: string, apiKey?: string | null) {
  return invoke<LlmValidationResult>('validate_llm_model', { baseUrl, model, apiKey: apiKey || null });
}

export function startDictation() {
  return invoke<void>('start_dictation');
}

export function stopDictation() {
  return invoke<void>('stop_dictation');
}

export function cancelDictation() {
  return invoke<void>('cancel_dictation');
}

export function listMicrophones() {
  return invoke<MicrophoneDevice[]>('list_microphones');
}

export function sherpaCatalog() {
  return invoke<SherpaModelInfo[]>('sherpa_catalog');
}

export function sherpaModelDir(alias: string) {
  return invoke<string>('sherpa_model_dir', { alias });
}

export function sherpaDefaultModelStatus() {
  return invoke<SherpaDefaultModelStatus>('sherpa_default_model_status');
}

export function sherpaPrepareDefaultModel() {
  return invoke<SherpaDefaultModelStatus>('sherpa_prepare_default_model');
}

export function onSherpaDownloadProgress(handler: (payload: SherpaDownloadProgress) => void) {
  return listen<SherpaDownloadProgress>('sherpa-download-progress', event => handler(event.payload));
}

export function listHistory() {
  return invoke<DictationSession[]>('list_history');
}

export function deleteHistoryEntry(id: string) {
  return invoke<void>('delete_history_entry', { id });
}

export function clearHistory() {
  return invoke<void>('clear_history');
}

export function listDictionary() {
  return invoke<DictionaryEntry[]>('list_dictionary');
}

export function addDictionaryEntry(phrase: string, note: string | null) {
  return invoke<DictionaryEntry>('add_dictionary_entry', { phrase, note });
}

export function removeDictionaryEntry(id: string) {
  return invoke<void>('remove_dictionary_entry', { id });
}

export function setDictionaryEntryEnabled(id: string, enabled: boolean) {
  return invoke<void>('set_dictionary_entry_enabled', { id, enabled });
}

export function listCorrectionRules() {
  return invoke<CorrectionRule[]>('list_correction_rules');
}

export function addCorrectionRule(pattern: string, replacement: string) {
  return invoke<CorrectionRule>('add_correction_rule', { pattern, replacement });
}

export function removeCorrectionRule(id: string) {
  return invoke<void>('remove_correction_rule', { id });
}

export function setCorrectionRuleEnabled(id: string, enabled: boolean) {
  return invoke<void>('set_correction_rule_enabled', { id, enabled });
}

export function listStyles() {
  return invoke<StyleProfile[]>('list_styles');
}

export function saveStyle(style: StyleProfile) {
  return invoke<StyleProfile>('save_style', { style });
}

export function resetBuiltinStyle(id: string) {
  return invoke<StyleProfile>('reset_builtin_style', { id });
}

export function onCapsule(handler: (payload: CapsulePayload) => void) {
  return listen<CapsulePayload>('capsule', event => handler(event.payload));
}

export function onHistoryChanged(handler: () => void) {
  return listen<void>('history:changed', () => handler());
}

export function onDictionaryChanged(handler: () => void) {
  return listen<void>('dictionary:changed', () => handler());
}
