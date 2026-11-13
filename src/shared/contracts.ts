/**
 * Focus OS — IPC contracts.
 *
 * Phase-1 hand-written mirror of the serde DTOs in `src-tauri/src/commands/`.
 * IMPORTANT: this file is replaced by tauri-specta codegen (Phase 2) so the
 * Rust and TypeScript layers can never drift. Treat it as read-only in the
 * meantime and keep field names/values byte-identical to the Rust side.
 */

export interface AppInfo {
  name: string
  version: string
  tauriVersion: string
  /** Minimum supported Rust version, from Cargo.toml `rust-version`. */
  msrv: string
}

export interface Health {
  ok: boolean
  db: 'ok' | 'error'
  migrationsApplied: number
  databasePath: string | null
  error: string | null
}

export interface InboxItem {
  id: string
  rawText: string
  kind: string
  status: string
  capturedAt: number
  analysis: AiAnalysisResult | null
}

export type AnalysisKind =
  | 'TASK'
  | 'IDEA'
  | 'PROJECT'
  | 'RESPONSIBILITY'
  | 'OBLIGATION'
  | 'RESEARCH'
  | 'EXPERIMENT'
  | 'AMBITION'
  | 'NOTE'

export type Actionability = 'ACTIONABLE' | 'MAYBE' | 'NOT_ACTIONABLE'

export interface BrainDumpAnalysis {
  kind: AnalysisKind
  project_guess: string | null
  goal_guess: string | null
  estimated_duration_minutes: number
  priority: number
  deadline: string | null
  actionability: Actionability
  next_step: string | null
  dependencies: string[] | null
  confidence: number
}

export interface AiAnalysisResult {
  provider: 'nim' | 'mock'
  analysis: BrainDumpAnalysis
}

export type ProjectStatus =
  | 'BACKLOG'
  | 'ACTIVE'
  | 'ON_HOLD'
  | 'COMPLETED'
  | 'ARCHIVED'

export interface Project {
  id: string
  name: string
  description: string | null
  status: ProjectStatus
  priority: number
  progress: number
  createdAt: number
  updatedAt: number
}

export interface ProjectInput {
  name: string
  description: string | null
  status: Exclude<ProjectStatus, 'ARCHIVED'>
  priority: number
}

export interface GithubRepository {
  id: number
  name: string
  fullName: string
  private: boolean
  htmlUrl: string
  description: string | null
  defaultBranch: string
  language: string | null
  stargazersCount: number
  forksCount: number
  topics: string[]
  updatedAt: string
}

export interface GithubSourceSummary {
  fullName: string
  defaultBranch: string
  description: string | null
  topics: string[]
  languages: Record<string, number>
  sourceFileCount: number
  testFileCount: number
  documentationFileCount: number
  manifestFiles: string[]
  hasReadme: boolean
  inspectedFileCount: number
  inspectedLines: number
  declarationsFound: number
}

export interface PortfolioEvaluation {
  readinessScore: number
  projectSummary: string
  strengths: string[]
  gaps: string[]
  resumeBullets: string[]
  verificationNotes: string[]
  confidence: number
}

export interface PortfolioReport {
  id: string
  repoFullName: string
  sourceSummary: GithubSourceSummary
  evaluation: PortfolioEvaluation
  draftMarkdown: string
  createdAt: number
  updatedAt: number
}

export interface Goal {
  id: string
  title: string
  description: string | null
  expectedOutcome: string | null
  successCriteria: string[]
  status: string
  progress: number
  priority: number
  targetDate: number | null
  createdAt: number
  updatedAt: number
}

export interface GoalSidecarEntry {
  id: string
  goalId: string
  source: 'USER' | 'FOCUS_OS' | 'INTEGRATION' | 'SYSTEM' | 'AI'
  entryType: 'CHECK_IN' | 'ACTIVITY' | 'ASSESSMENT'
  content: string
  applicationName: string | null
  windowTitle: string | null
  domain: string | null
  observedStart: number | null
  observedEnd: number | null
  durationSeconds: number | null
  confidence: number
  reviewStatus: 'CONFIRMED' | 'PENDING' | 'REJECTED'
  createdAt: number
}

export type TaskStatus =
  | 'INBOX'
  | 'TODO'
  | 'SCHEDULED'
  | 'IN_PROGRESS'
  | 'PAUSED'
  | 'COMPLETED'
  | 'CANCELLED'

export interface Task {
  id: string
  projectId: string | null
  goalId: string | null
  title: string
  description: string | null
  status: TaskStatus
  priority: number
  estimatedMinutes: number
  actualMinutes: number
  requiresDeepWork: boolean
  createdAt: number
  updatedAt: number
}

export interface PlannedTask {
  taskId: string
  title: string
  description: string | null
  priority: number
  estimatedMinutes: number
  score: number
  deadline: number | null
  fitsCapacity: boolean
}

export interface DailyPlan {
  id: string
  planDate: string
  version: number
  capacityMinutes: number
  plannedWorkMinutes: number
  overflowMinutes: number
  workloadReserveMinutes: number
  tasks: PlannedTask[]
  blockedTasks: PlannedTask[]
  createdAt: number
}

export interface FocusSession {
  id: string
  taskId: string
  taskTitle: string
  startTime: number
}

export interface AiSettings {
  enabled: boolean
  provider: 'nim' | 'mock'
  baseUrl: string | null
  model: string | null
  apiKeyStored: boolean
}

export interface ScheduleSettings {
  workingHoursStart: string // 'HH:MM' 24h
  workingHoursEnd: string
  maxDailyMinutes: number
  reserveRatio: number // 0..1 protected unfilled capacity
}

export interface AppearanceSettings {
  theme: 'dark'
}

export interface Settings {
  ai: AiSettings
  schedule: ScheduleSettings
  appearance: AppearanceSettings
}