import { invoke } from '@tauri-apps/api/core'
import type { AppInfo, Health } from '../shared/contracts'

/** True when running inside the Tauri webview, false in plain Vite/vitest. */
export const isTauri = (): boolean => '__TAURI_INTERNALS__' in window

/**
 * Thin typed wrapper around the Rust command surface.
 * Every method maps 1:1 to a `#[command]` in `src-tauri/src/commands/`.
 */
export const bridge = {
  appInfo: (): Promise<AppInfo> => invoke('app_info'),
  health: (): Promise<Health> => invoke('health'),
  settingsGet: (key: string): Promise<string | null> =>
    invoke('settings_get', { key }),
  settingsSet: (key: string, value: string): Promise<void> =>
    invoke('settings_set', { key, value }),
  settingsAll: (): Promise<Record<string, string>> => invoke('settings_all'),
} as const