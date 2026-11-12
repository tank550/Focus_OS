import { create } from 'zustand'
import { bridge, isTauri } from '../lib/tauri'
import type { Project, ProjectInput } from '../shared/contracts'

interface ProjectsStore {
  projects: Project[]
  loaded: boolean
  mutating: boolean
  error: string | null
  load: () => Promise<void>
  update: (id: string, input: ProjectInput) => Promise<boolean>
  archive: (id: string) => Promise<boolean>
}

export const useProjectsStore = create<ProjectsStore>((set, get) => ({
  projects: [],
  loaded: false,
  mutating: false,
  error: null,

  load: async () => {
    if (!isTauri()) {
      set({ loaded: true })
      return
    }
    try {
      const projects = await bridge.projectsList()
      set({ projects, loaded: true, error: null })
    } catch (error) {
      set({
        loaded: true,
        error: error instanceof Error ? error.message : String(error),
      })
    }
  },

  update: async (id, input) => {
    if (!isTauri() || get().mutating) return false
    set({ mutating: true, error: null })
    try {
      await bridge.projectsUpdate(id, input)
      set((state) => ({
        projects: state.projects.map((project) =>
          project.id === id
            ? {
                ...project,
                ...input,
                name: input.name.trim(),
                description: input.description?.trim() || null,
                updatedAt: Date.now(),
              }
            : project,
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
      await bridge.projectsArchive(id)
      set((state) => ({
        projects: state.projects.filter((project) => project.id !== id),
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
