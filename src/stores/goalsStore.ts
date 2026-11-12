import { create } from 'zustand'
import { bridge, isTauri } from '../lib/tauri'
import type { Goal, GoalSidecarEntry } from '../shared/contracts'

interface GoalsStore {
  goals: Goal[]
  sidecarByGoal: Record<string, GoalSidecarEntry[]>
  loadingSidecar: Record<string, boolean>
  loaded: boolean
  saving: boolean
  error: string | null
  load: () => Promise<void>
  loadSidecar: (goalId: string) => Promise<void>
  create: (input: {
    title: string
    description: string | null
    expectedOutcome: string | null
    successCriteria: string[]
  }) => Promise<boolean>
  addCheckIn: (goalId: string, content: string) => Promise<boolean>
}

export const useGoalsStore = create<GoalsStore>((set, get) => ({
  goals: [],
  sidecarByGoal: {},
  loadingSidecar: {},
  loaded: false,
  saving: false,
  error: null,

  load: async () => {
    if (!isTauri()) {
      set({ loaded: true })
      return
    }
    try {
      const goals = await bridge.goalsList()
      set({ goals, loaded: true, error: null })
    } catch (error) {
      set({
        loaded: true,
        error: error instanceof Error ? error.message : String(error),
      })
    }
  },

  loadSidecar: async (goalId) => {
    if (!isTauri() || get().loadingSidecar[goalId]) return
    set((state) => ({
      loadingSidecar: { ...state.loadingSidecar, [goalId]: true },
      error: null,
    }))
    try {
      const entries = await bridge.goalSidecarList(goalId)
      set((state) => ({
        sidecarByGoal: { ...state.sidecarByGoal, [goalId]: entries },
        loadingSidecar: { ...state.loadingSidecar, [goalId]: false },
      }))
    } catch (error) {
      set((state) => ({
        loadingSidecar: { ...state.loadingSidecar, [goalId]: false },
        error: error instanceof Error ? error.message : String(error),
      }))
    }
  },

  create: async (input) => {
    if (!isTauri() || get().saving) return false
    set({ saving: true, error: null })
    try {
      const goal = await bridge.goalsCreate(input)
      set((state) => ({
        goals: [...state.goals, goal],
        saving: false,
      }))
      return true
    } catch (error) {
      set({
        saving: false,
        error: error instanceof Error ? error.message : String(error),
      })
      return false
    }
  },

  addCheckIn: async (goalId, content) => {
    if (!isTauri() || get().saving) return false
    set({ saving: true, error: null })
    try {
      const entry = await bridge.goalSidecarAdd(goalId, content)
      set((state) => ({
        sidecarByGoal: {
          ...state.sidecarByGoal,
          [goalId]: [entry, ...(state.sidecarByGoal[goalId] ?? [])],
        },
        saving: false,
      }))
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
