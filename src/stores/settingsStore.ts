import { create } from 'zustand'
import { bridge, isTauri } from '../lib/tauri'
import type { AiSettings, ScheduleSettings, Settings } from '../shared/contracts'

/** Mirrors the Rust defaults in `application::settings::Settings::default()`. */
export const DEFAULT_SETTINGS: Settings = {
  ai: {
    enabled: true,
    provider: 'nim',
    baseUrl: 'https://integrate.api.nvidia.com',
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
  error: string | null
  load: () => Promise<void>
  saveAi: (ai: AiSettings) => Promise<void>
  saveSchedule: (schedule: ScheduleSettings) => Promise<void>
  storeAiApiKey: (apiKey: string) => Promise<void>
  removeAiApiKey: () => Promise<void>
}

export const useSettingsStore = create<SettingsStore>((set) => ({
  settings: DEFAULT_SETTINGS,
  loaded: false,
  error: null,

  load: async () => {
    if (!isTauri()) {
      set({ loaded: true })
      return
    }
    try {
      const [all, apiKeyStored] = await Promise.all([
        bridge.settingsAll(),
        bridge.aiApiKeyStatus(),
      ])
      set({
        settings: {
          ai: { ...parseBlock(all.ai, DEFAULT_SETTINGS.ai), apiKeyStored },
          schedule: parseBlock(all.schedule, DEFAULT_SETTINGS.schedule),
          appearance: parseBlock(
            all.appearance,
            DEFAULT_SETTINGS.appearance,
          ),
        },
        loaded: true,
        error: null,
      })
    } catch (error) {
      set({
        loaded: true,
        error: error instanceof Error ? error.message : String(error),
      })
    }
  },

  saveAi: async (ai) => {
    await bridge.settingsSet('ai', JSON.stringify(ai))
    set((state) => ({
      settings: { ...state.settings, ai },
      error: null,
    }))
  },

  saveSchedule: async (schedule) => {
    await bridge.settingsSet('schedule', JSON.stringify(schedule))
    set((state) => ({
      settings: { ...state.settings, schedule },
      error: null,
    }))
  },

  storeAiApiKey: async (apiKey) => {
    await bridge.aiApiKeyStore(apiKey)
    set((state) => ({
      settings: {
        ...state.settings,
        ai: { ...state.settings.ai, apiKeyStored: true },
      },
      error: null,
    }))
  },

  removeAiApiKey: async () => {
    await bridge.aiApiKeyRemove()
    set((state) => ({
      settings: {
        ...state.settings,
        ai: { ...state.settings.ai, apiKeyStored: false },
      },
      error: null,
    }))
  },
}))