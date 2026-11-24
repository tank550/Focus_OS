import { useCallback, useEffect, useState } from 'react'
import { BarChart3, CheckCheck, Clock3, RefreshCw, Timer } from 'lucide-react'
import { Card } from '../../components/ui/Card'
import { EmptyState } from '../../components/ui/EmptyState'
import { bridge } from '../../lib/tauri'
import type { AnalyticsSummary } from '../../shared/contracts'

function formatDuration(minutes: number) {
  const hours = Math.floor(minutes / 60)
  const remainingMinutes = minutes % 60
  if (hours === 0) return `${remainingMinutes} min`
  if (remainingMinutes === 0) return `${hours} h`
  return `${hours} h ${remainingMinutes} min`
}

function formatSessionDate(timestamp: number) {
  return new Intl.DateTimeFormat(undefined, {
    day: 'numeric',
    month: 'short',
    hour: '2-digit',
    minute: '2-digit',
  }).format(new Date(timestamp))
}

export function AnalyticsScreen() {
  const [summary, setSummary] = useState<AnalyticsSummary | null>(null)
  const [loading, setLoading] = useState(true)
  const [refreshing, setRefreshing] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const refresh = useCallback(async () => {
    setRefreshing(true)
    setError(null)
    try {
      setSummary(await bridge.analyticsSummary())
    } catch (reason) {
      setError(String(reason))
    } finally {
      setLoading(false)
      setRefreshing(false)
    }
  }, [])

  useEffect(() => {
    void refresh()
  }, [refresh])

  const maxProjectMinutes = Math.max(
    1,
    ...(summary?.projectFocusTimes.map((project) => project.focusMinutes) ?? []),
  )

  return (
    <div className="flex flex-col gap-6">
      <header className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <p className="text-xs uppercase tracking-wider text-text-muted">
            Analytics
          </p>
          <h1 className="mt-1 text-2xl font-semibold">Votre activité</h1>
          <p className="mt-2 text-sm text-text-muted">
            Sessions de concentration et tâches terminées sur les 30 derniers
            jours.
          </p>
        </div>
        <button
          type="button"
          disabled={refreshing}
          onClick={() => void refresh()}
          className="inline-flex items-center gap-2 rounded-md border border-border px-3 py-2 text-sm text-text-muted hover:text-text disabled:opacity-50"
        >
          <RefreshCw
            className={`h-4 w-4 ${refreshing ? 'animate-spin' : ''}`}
          />
          Actualiser
        </button>
      </header>

      {error && (
        <p
          role="alert"
          className="rounded-md border border-danger/30 px-4 py-3 text-sm text-danger"
        >
          Impossible de charger les statistiques : {error}
        </p>
      )}

      {loading ? (
        <p role="status" className="py-6 text-sm text-text-muted">
          Chargement des statistiques…
        </p>
      ) : summary ? (
        <>
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4">
            <Card>
              <div className="flex items-start justify-between gap-3">
                <div>
                  <p className="text-xs text-text-muted">Temps concentré</p>
                  <p className="mt-2 text-2xl font-semibold">
                    {formatDuration(summary.focusMinutes)}
                  </p>
                </div>
                <Clock3 className="h-5 w-5 text-accent" />
              </div>
            </Card>
            <Card>
              <div className="flex items-start justify-between gap-3">
                <div>
                  <p className="text-xs text-text-muted">Sessions terminées</p>
                  <p className="mt-2 text-2xl font-semibold">
                    {summary.sessionCount}
                  </p>
                </div>
                <Timer className="h-5 w-5 text-accent" />
              </div>
            </Card>
            <Card>
              <div className="flex items-start justify-between gap-3">
                <div>
                  <p className="text-xs text-text-muted">Durée moyenne</p>
                  <p className="mt-2 text-2xl font-semibold">
                    {formatDuration(summary.averageSessionMinutes)}
                  </p>
                </div>
                <BarChart3 className="h-5 w-5 text-accent" />
              </div>
            </Card>
            <Card>
              <div className="flex items-start justify-between gap-3">
                <div>
                  <p className="text-xs text-text-muted">Tâches terminées</p>
                  <p className="mt-2 text-2xl font-semibold">
                    {summary.completedTasks}
                  </p>
                </div>
                <CheckCheck className="h-5 w-5 text-accent" />
              </div>
            </Card>
          </div>

          <div className="grid grid-cols-1 gap-6 xl:grid-cols-2">
            <Card title="Temps par projet">
              {summary.projectFocusTimes.length === 0 ? (
                <EmptyState
                  icon={<BarChart3 className="h-8 w-8" />}
                  title="Pas encore de temps enregistré"
                  hint="Terminez une session de concentration pour voir la répartition."
                />
              ) : (
                <ul className="flex flex-col gap-4">
                  {summary.projectFocusTimes.map((project) => (
                    <li
                      key={project.projectId ?? 'no-project'}
                      className="flex flex-col gap-2"
                    >
                      <div className="flex items-center justify-between gap-3 text-sm">
                        <span className="truncate">{project.projectName}</span>
                        <span className="shrink-0 text-text-muted">
                          {formatDuration(project.focusMinutes)}
                        </span>
                      </div>
                      <div
                        role="progressbar"
                        aria-label={`Temps consacré à ${project.projectName}`}
                        aria-valuemin={0}
                        aria-valuemax={maxProjectMinutes}
                        aria-valuenow={project.focusMinutes}
                        className="h-2 overflow-hidden rounded-full bg-surface-2"
                      >
                        <div
                          className="h-full rounded-full bg-accent"
                          style={{
                            width: `${(project.focusMinutes / maxProjectMinutes) * 100}%`,
                          }}
                        />
                      </div>
                    </li>
                  ))}
                </ul>
              )}
            </Card>

            <Card title="Sessions récentes">
              {summary.recentSessions.length === 0 ? (
                <EmptyState
                  icon={<Clock3 className="h-8 w-8" />}
                  title="Aucune session récente"
                  hint="Vos sessions terminées apparaîtront ici."
                />
              ) : (
                <ul className="flex flex-col divide-y divide-border">
                  {summary.recentSessions.map((session) => (
                    <li
                      key={session.id}
                      className="flex items-center justify-between gap-4 py-3 first:pt-0 last:pb-0"
                    >
                      <div className="min-w-0">
                        <p className="truncate text-sm font-medium">
                          {session.taskTitle}
                        </p>
                        <p className="mt-1 truncate text-xs text-text-muted">
                          {session.projectName ?? 'Sans projet'} ·{' '}
                          {formatSessionDate(session.startTime)}
                        </p>
                      </div>
                      <span className="shrink-0 text-sm text-text-muted">
                        {formatDuration(session.durationMinutes)}
                      </span>
                    </li>
                  ))}
                </ul>
              )}
            </Card>
          </div>
        </>
      ) : null}
    </div>
  )
}
