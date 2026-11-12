import { invoke } from '@tauri-apps/api/core'
import type {
  AppInfo,
  AiAnalysisResult,
  Health,
  Goal,
  GoalSidecarEntry,
  FocusSession,
  InboxItem,
  Project,
  ProjectInput,
  Task,
} from '../shared/contracts'

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
  inboxAnalyze: (id: string): Promise<AiAnalysisResult> =>
    invoke('inbox_analyze', { id }),
  projectsList: (): Promise<Project[]> => invoke('projects_list'),
  projectsUpdate: (id: string, input: ProjectInput): Promise<void> =>
    invoke('projects_update', { id, ...input }),
  projectsArchive: (id: string): Promise<void> =>
    invoke('projects_archive', { id }),
  goalsList: (): Promise<Goal[]> => invoke('goals_list'),
  goalsCreate: (input: {
    title: string
    description: string | null
    expectedOutcome: string | null
    successCriteria: string[]
  }): Promise<Goal> => invoke('goals_create', input),
  goalSidecarList: (goalId: string): Promise<GoalSidecarEntry[]> =>
    invoke('goal_sidecar_list', { goalId }),
  goalSidecarAdd: (
    goalId: string,
    content: string,
  ): Promise<GoalSidecarEntry> =>
    invoke('goal_sidecar_add', { goalId, content }),
  tasksListOpen: (): Promise<Task[]> => invoke('tasks_list_open'),
  tasksCurrentFocus: (): Promise<FocusSession | null> =>
    invoke('tasks_current_focus'),
  tasksStartFocus: (taskId: string): Promise<FocusSession> =>
    invoke('tasks_start_focus', { taskId }),
  tasksPauseFocus: (taskId: string): Promise<void> =>
    invoke('tasks_pause_focus', { taskId }),
  tasksComplete: (taskId: string): Promise<void> =>
    invoke('tasks_complete', { taskId }),
  settingsGet: (key: string): Promise<string | null> =>
    invoke('settings_get', { key }),
  settingsSet: (key: string, value: string): Promise<void> =>
    invoke('settings_set', { key, value }),
  settingsAll: (): Promise<Record<string, string>> => invoke('settings_all'),
  aiApiKeyStatus: (): Promise<boolean> => invoke('ai_api_key_status'),
  aiApiKeyStore: (apiKey: string): Promise<void> =>
    invoke('ai_api_key_store', { apiKey }),
  aiApiKeyRemove: (): Promise<void> => invoke('ai_api_key_remove'),
} as const