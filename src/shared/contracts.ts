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