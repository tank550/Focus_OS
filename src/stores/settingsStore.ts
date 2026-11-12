import { create } from 'zustand'
import { bridge, isTauri } from '../lib/tauri'
import type { Settings } from '../shared/contracts'

/** Mirrors the Rust defaults in `application::settings::Settings::default()`. */
export const DEFAULT_SETTINGS: Settings = {
  ai: {
    enabled: true,
    provider: 'mock',
    baseUrl: null,
    model: null,
    apiKeyStored: false,
  },
  schedule: {
    workingHoursStart: '09:00',
    workingHoursEnd: '18:00',
    maxDailyMinutes: 480,
    reserveRatio: 0.15,
  },
  appearance: { theme: 'dark' },
}

/** Minimal structural guard until Zod lands with the AI layer (Phase 4). */
function parseBlock<T>(raw: string | undefined, fallback: T): T {
  if (!raw) return fallback
  try {
    const parsed = JSON.parse(raw) as T
    return parsed && typeof parsed === 'object' ? parsed : fallback
  } catch {
    return fallback
  }
}

interface SettingsStore {
  settings: Settings
  loaded: boolean
  load: () => Promise<void>
}

export const useSettingsStore = create<SettingsStore>((set) => ({
  settings: DEFAULT_SETTINGS,
  loaded: false,

  load: async () => {
    if (!isTauri()) {
      set({ loaded: true })
      return
    }
    try {
      const all = await bridge.settingsAll()
      set({
        settings: {
          ai: parseBlock(all.ai, DEFAULT_SETTINGS.ai),
          schedule: parseBlock(all.schedule, DEFAULT_SETTINGS.schedule),
          appearance: parseBlock(
            all.appearance,
            DEFAULT_SETTINGS.appearance,
          ),
        },
        loaded: true,
      })
    } catch {
      set({ loaded: true })
    }
  },
}))