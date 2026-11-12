import { Card } from '../../components/ui/Card'
import { minutesToLabel } from '../../lib/date'
import { useAppStore } from '../../stores/appStore'
import { useSettingsStore } from '../../stores/settingsStore'

export function SettingsScreen() {
  const { appInfo, health, inTauri } = useAppStore()
  const { settings } = useSettingsStore()

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
          <p className="text-sm">
            Provider:{' '}
            <span className="font-mono text-text">{settings.ai.provider}</span>
            {settings.ai.model && (
              <>
                {' '}
                · model{' '}
                <span className="font-mono text-text">{settings.ai.model}</span>
              </>
            )}
          </p>
          <p className="mt-2 text-xs leading-relaxed text-text-muted">
            NVIDIA NIM credentials are stored in the OS keyring and never
            written to disk in plain text. Configuration UI lands in Phase 4.
          </p>
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