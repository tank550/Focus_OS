import { invoke } from '@tauri-apps/api/core'
import type { AppInfo, Health, InboxItem } from '../shared/contracts'

/** True when running inside the Tauri webview, false in plain Vite/vitest. */
export const isTauri = (): boolean => '__TAURI_INTERNALS__' in window

/**
 * Thin typed wrapper around the Rust command surface.
 * Every method maps 1:1 to a `#[command]` in `src-tauri/src/commands/`.
 */
export const bridge = {
  appInfo: (): Promise<AppInfo> => invoke('app_info'),
  health: (): Promise<Health> => invoke('health'),
  inboxList: (): Promise<InboxItem[]> => invoke('inbox_list'),
  inboxCapture: (rawText: string): Promise<InboxItem> =>
    invoke('inbox_capture', { rawText }),
  inboxUpdate: (id: string, rawText: string): Promise<void> =>
    invoke('inbox_update', { id, rawText }),
  inboxArchive: (id: string): Promise<void> =>
    invoke('inbox_archive', { id }),
  settingsGet: (key: string): Promise<string | null> =>
    invoke('settings_get', { key }),
  settingsSet: (key: string, value: string): Promise<void> =>
    invoke('settings_set', { key, value }),
  settingsAll: (): Promise<Record<string, string>> => invoke('settings_all'),
} as const