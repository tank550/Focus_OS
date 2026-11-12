import { create } from 'zustand'
import { bridge, isTauri } from '../lib/tauri'
import type { AppInfo, Health } from '../shared/contracts'

interface AppStore {
  inTauri: boolean
  appInfo: AppInfo | null
  health: Health | null
  error: string | null
  loaded: boolean
  load: () => Promise<void>
}

export const useAppStore = create<AppStore>((set) => ({
  inTauri: isTauri(),
  appInfo: null,
  health: null,
  error: null,
  loaded: false,

  load: async () => {
    if (!isTauri()) {
      set({ loaded: true })
      return
    }
    try {
      const [appInfo, health] = await Promise.all([
        bridge.appInfo(),
        bridge.health(),
      ])
      set({
        appInfo,
        health,
        error: health.ok ? null : health.error,
        loaded: true,
      })
    } catch (e) {
      set({ error: e instanceof Error ? e.message : String(e), loaded: true })
    }
  },
}))