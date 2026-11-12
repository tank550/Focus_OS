import { useEffect, useState } from 'react'
import { Card } from '../../components/ui/Card'
import { minutesToLabel } from '../../lib/date'
import { useAppStore } from '../../stores/appStore'
import { useSettingsStore } from '../../stores/settingsStore'
import type { AiSettings } from '../../shared/contracts'

const inputClass =
  'w-full rounded-md border border-border bg-surface-2 px-3 py-2 text-sm outline-none focus:border-accent'

export function SettingsScreen() {
  const { appInfo, health, inTauri } = useAppStore()
  const {
    settings,
    error: settingsError,
    saveAi,
    storeAiApiKey,
    removeAiApiKey,
  } = useSettingsStore()
  const [aiConfig, setAiConfig] = useState<AiSettings>(settings.ai)
  const [apiKey, setApiKey] = useState('')
  const [savingAi, setSavingAi] = useState(false)
  const [aiMessage, setAiMessage] = useState<string | null>(null)

  useEffect(() => {
    setAiConfig(settings.ai)
  }, [settings.ai])

  const saveConfiguration = async () => {
    setSavingAi(true)
    setAiMessage(null)
    try {
      await saveAi({
        ...aiConfig,
        baseUrl: aiConfig.baseUrl?.trim() || null,
        model: aiConfig.model?.trim() || null,
      })
      setAiMessage('AI configuration saved.')
    } catch (reason) {
      setAiMessage(String(reason))
    } finally {
      setSavingAi(false)
    }
  }

  const saveApiKey = async () => {
    setSavingAi(true)
    setAiMessage(null)
    try {
      await storeAiApiKey(apiKey)
      setApiKey('')
      setAiMessage('API key stored in the operating system credential store.')
    } catch (reason) {
      setAiMessage(String(reason))
    } finally {
      setSavingAi(false)
    }
  }

  const clearApiKey = async () => {
    setSavingAi(true)
    setAiMessage(null)
    try {
      await removeAiApiKey()
      setApiKey('')
      setAiMessage('Stored API key removed.')
    } catch (reason) {
      setAiMessage(String(reason))
    } finally {
      setSavingAi(false)
    }
  }

  return (
    <div className="flex flex-col gap-6">
      <header>
        <p className="text-xs uppercase tracking-wider text-text-muted">
          Settings
        </p>
        <h1 className="mt-1 text-2xl font-semibold">Settings</h1>
      </header>

      <div className="grid grid-cols-1 gap-6 lg:grid-cols-2">
        <Card title="AI provider">
          <div className="flex flex-col gap-4">
            <label className="flex flex-col gap-1.5 text-xs text-text-muted">
              Provider
              <select
                className={inputClass}
                value={aiConfig.provider}
                onChange={(event) =>
                  setAiConfig({
                    ...aiConfig,
                    provider: event.target.value as AiSettings['provider'],
                  })
                }
                disabled={!inTauri || savingAi}
              >
                <option value="nim">NVIDIA NIM (real AI)</option>
                <option value="mock">Mock (fixed demo response; no AI)</option>
              </select>
            </label>

            {aiConfig.provider === 'nim' ? (
              <>
                <label className="flex flex-col gap-1.5 text-xs text-text-muted">
                  NIM API base URL
                  <input
                    className={inputClass}
                    value={aiConfig.baseUrl ?? ''}
                    placeholder="https://integrate.api.nvidia.com"
                    onChange={(event) =>
                      setAiConfig({ ...aiConfig, baseUrl: event.target.value })
                    }
                    disabled={!inTauri || savingAi}
                  />
                </label>
                <label className="flex flex-col gap-1.5 text-xs text-text-muted">
                  Model ID from your NVIDIA catalog
                  <input
                    className={inputClass}
                    value={aiConfig.model ?? ''}
                    placeholder="e.g. model ID shown in your NIM catalog"
                    onChange={(event) =>
                      setAiConfig({ ...aiConfig, model: event.target.value })
                    }
                    disabled={!inTauri || savingAi}
                  />
                </label>
                <div className="rounded-md border border-border bg-surface-2 p-3">
                  <p className="text-xs text-text-muted">
                    API key:{' '}
                    {settings.ai.apiKeyStored
                      ? 'stored in the OS credential store'
                      : 'not configured'}
                  </p>
                  <label className="mt-3 flex flex-col gap-1.5 text-xs text-text-muted">
                    NVIDIA API key
                    <input
                      className={inputClass}
                      type="password"
                      autoComplete="new-password"
                      value={apiKey}
                      onChange={(event) => setApiKey(event.target.value)}
                      disabled={!inTauri || savingAi}
                    />
                  </label>
                  <div className="mt-3 flex flex-wrap gap-2">
                    <button
                      type="button"
                      className="rounded-md border border-border px-3 py-1.5 text-xs text-text-muted hover:text-text disabled:opacity-40"
                      onClick={() => void saveApiKey()}
                      disabled={!inTauri || savingAi || !apiKey.trim()}
                    >
                      Store API key securely
                    </button>
                    {settings.ai.apiKeyStored && (
                      <button
                        type="button"
                        className="rounded-md border border-danger/30 px-3 py-1.5 text-xs text-danger hover:bg-danger/10 disabled:opacity-40"
                        onClick={() => void clearApiKey()}
                        disabled={!inTauri || savingAi}
                      >
                        Remove API key
                      </button>
                    )}
                  </div>
                </div>
                <p className="text-xs leading-relaxed text-text-muted">
                  A note is sent to NVIDIA only when you explicitly choose to
                  analyze it. The API key is never written to SQLite.
                </p>
              </>
            ) : (
              <p className="text-xs leading-relaxed text-text-muted">
                Mock is a fixed test response, not a model. It sends no note
                content anywhere and will not create tasks or projects.
              </p>
            )}

            <div className="flex flex-wrap items-center gap-3">
              <button
                type="button"
                className="rounded-md bg-accent px-4 py-2 text-sm font-medium text-white hover:bg-accent/90 disabled:opacity-40"
                onClick={() => void saveConfiguration()}
                disabled={!inTauri || savingAi}
              >
                {savingAi ? 'Saving…' : 'Save AI settings'}
              </button>
              {aiMessage && (
                <p className="text-xs text-text-muted" role="status">
                  {aiMessage}
                </p>
              )}
              {settingsError && (
                <p className="text-xs text-danger" role="alert">
                  {settingsError}
                </p>
              )}
            </div>
          </div>
        </Card>

        <Card title="Schedule">
          <p className="text-sm">
            Working hours {settings.schedule.workingHoursStart}–
            {settings.schedule.workingHoursEnd} · daily cap{' '}
            {minutesToLabel(settings.schedule.maxDailyMinutes)} · reserve{' '}
            {Math.round(settings.schedule.reserveRatio * 100)}%
          </p>
          <p className="mt-2 text-xs leading-relaxed text-text-muted">
            Workload guardrails feed the deterministic scheduler (Phase 3).
          </p>
        </Card>

        <Card title="Application">
          {inTauri && appInfo ? (
            <p className="text-sm">
              {appInfo.name} v{appInfo.version} · Tauri {appInfo.tauriVersion}
            </p>
          ) : (
            <p className="text-sm text-text-muted">
              Running in browser preview mode.
            </p>
          )}
          {health && (
            <p className="mt-2 text-xs text-text-muted">
              Database:{' '}
              <span className={health.ok ? 'text-success' : 'text-danger'}>
                {health.ok ? health.db : health.error}
              </span>{' '}
              · {health.migrationsApplied} migration(s) applied
              {health.databasePath && (
                <span className="block break-all">{health.databasePath}</span>
              )}
            </p>
          )}
        </Card>

        <Card title="Appearance">
          <p className="text-sm">
            Theme:{' '}
            <span className="font-mono text-text">{settings.appearance.theme}</span>
          </p>
        </Card>
      </div>
    </div>
  )
}