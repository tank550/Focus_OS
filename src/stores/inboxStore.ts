import { create } from 'zustand'
import { bridge, isTauri } from '../lib/tauri'
import type { InboxItem } from '../shared/contracts'

interface InboxStore {
  items: InboxItem[]
  loaded: boolean
  saving: boolean
  error: string | null
  load: () => Promise<void>
  capture: (rawText: string) => Promise<boolean>
}

export const useInboxStore = create<InboxStore>((set) => ({
  items: [],
  loaded: false,
  saving: false,
  error: null,

  load: async () => {
    if (!isTauri()) {
      set({ loaded: true })
      return
    }
    try {
      const items = await bridge.inboxList()
      set({ items, loaded: true, error: null })
    } catch (error) {
      set({
        loaded: true,
        error: error instanceof Error ? error.message : String(error),
      })
    }
  },

  capture: async (rawText) => {
    if (!isTauri()) return false
    set({ saving: true, error: null })
    try {
      const item = await bridge.inboxCapture(rawText)
      set((state) => ({ items: [item, ...state.items], saving: false }))
      return true
    } catch (error) {
      set({
        saving: false,
        error: error instanceof Error ? error.message : String(error),
      })
      return false
    }
  },
}))
