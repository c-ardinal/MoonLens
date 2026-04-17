/**
 * Thin typed wrappers around Tauri `invoke` calls.
 * Keeping all IPC surface in one file makes it easier to mock during tests
 * and to see everything the frontend can ask the backend to do.
 */

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

export type Settings = Record<string, unknown>;

export interface TaskResult<O = unknown> {
  task: string;
  output: O;
  used_provider: string;
  via_cloud: boolean;
  latency_ms: number;
  fallback_reasons: string[];
}

export interface PluginDescriptor {
  id: string;
  name: string;
  version: string;
  author: string;
  license: string;
  kind: 'wasm' | 'sidecar';
  enabled: boolean;
  provides: string[];
  permissions_summary: string[];
  path: string;
}

export async function getSettings(): Promise<Settings> {
  return await invoke<Settings>('get_settings');
}

export async function setSettings(next: Settings): Promise<void> {
  await invoke('set_settings', { new: next });
}

export async function getSettingsSchema(): Promise<unknown> {
  return await invoke('get_settings_schema');
}

export async function listPlugins(): Promise<PluginDescriptor[]> {
  return await invoke<PluginDescriptor[]>('list_plugins');
}

export async function setPluginEnabled(id: string, enabled: boolean): Promise<void> {
  await invoke('set_plugin_enabled', { id, enabled });
}

export async function runTranslate(args: {
  text: string;
  source_language: string;
  target_language: string;
}): Promise<TaskResult<{ Text?: string }>> {
  return await invoke('run_translate', { args });
}

export async function runOcr(
  imagePng: Uint8Array,
  languages: string[]
): Promise<TaskResult> {
  return await invoke('run_ocr', { imagePng: Array.from(imagePng), languages });
}

export async function runIntent(imagePng: Uint8Array): Promise<string> {
  return await invoke<string>('run_intent', { imagePng: Array.from(imagePng) });
}

export async function runQr(imagePng: Uint8Array): Promise<TaskResult> {
  return await invoke('run_qr', { imagePng: Array.from(imagePng) });
}

export async function runReverseImageSearch(imageUrl: string): Promise<TaskResult> {
  return await invoke('run_reverse_image_search', { imageUrl });
}

export async function captureRegion(args: {
  monitor_index: number;
  x: number;
  y: number;
  w: number;
  h: number;
}): Promise<{ image_png: number[] }> {
  return await invoke('capture_region_cmd', { args });
}

export async function onHotkey(handler: (action: string) => void): Promise<UnlistenFn> {
  return await listen<string>('moonlens://hotkey', (e) => handler(e.payload));
}
