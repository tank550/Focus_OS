import { create } from 'zustand'
import { bridge, isTauri } from '../lib/tauri'
import type { InboxItem } from '../shared/contracts'

interface InboxStore {
  items: InboxItem[]
  loaded: boolean
  saving: boolean
  mutating: boolean
  error: string | null
  load: () => Promise<void>
  capture: (rawText: string) => Promise<boolean>
  update: (id: string, rawText: string) => Promise<boolean>
  archive: (id: string) => Promise<boolean>
}

export const useInboxStore = create<InboxStore>((set, get) => ({
  items: [],
  loaded: false,
  saving: false,
  mutating: false,
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
    if (!isTauri() || get().saving) return false
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

  update: async (id, rawText) => {
    if (!isTauri() || get().mutating) return false
    set({ mutating: true, error: null })
    try {
      await bridge.inboxUpdate(id, rawText)
      set((state) => ({
        items: state.items.map((item) =>
          item.id === id ? { ...item, rawText } : item,
        ),
        mutating: false,
      }))
      return true
    } catch (error) {
      set({
        mutating: false,
        error: error instanceof Error ? error.message : String(error),
      })
      return false
    }
  },

  archive: async (id) => {
    if (!isTauri() || get().mutating) return false
    set({ mutating: true, error: null })
    try {
      await bridge.inboxArchive(id)
      set((state) => ({
        items: state.items.filter((item) => item.id !== id),
        mutating: false,
      }))
      return true
    } catch (error) {
      set({
        mutating: false,
        error: error instanceof Error ? error.message : String(error),
      })
      return false
    }
  },
}))
