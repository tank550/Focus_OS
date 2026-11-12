import { useCallback, useEffect, useState } from 'react'
import { Check, Clock3, Pause, Play, Timer } from 'lucide-react'
import { Card } from '../../components/ui/Card'
import { EmptyState } from '../../components/ui/EmptyState'
import { localDateKey } from '../../lib/date'
import { bridge } from '../../lib/tauri'
import type { FocusSession, Task } from '../../shared/contracts'

function formatDuration(totalSeconds: number) {
  const hours = Math.floor(totalSeconds / 3600)
  const minutes = Math.floor((totalSeconds % 3600) / 60)
  const seconds = totalSeconds % 60
  return [hours, minutes, seconds]
    .map((part) => String(part).padStart(2, '0'))
    .join(':')
}

export function TodayScreen() {
  const [tasks, setTasks] = useState<Task[]>([])
  const [focus, setFocus] = useState<FocusSession | null>(null)
  const [loading, setLoading] = useState(true)
  const [busyTaskId, setBusyTaskId] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [now, setNow] = useState(Date.now())

  const refresh = useCallback(async () => {
    try {
      const [nextTasks, nextFocus] = await Promise.all([
        bridge.tasksListOpen(),
        bridge.tasksCurrentFocus(),
      ])
      setTasks(nextTasks)
      setFocus(nextFocus)
      setError(null)
    } catch (reason) {
      setError(String(reason))
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    void refresh()
  }, [refresh])

  useEffect(() => {
    if (!focus) return
    const timer = window.setInterval(() => setNow(Date.now()), 1000)
    return () => window.clearInterval(timer)
  }, [focus])

  const runAction = async (
    taskId: string,
    action: () => Promise<unknown>,
  ) => {
    setBusyTaskId(taskId)
    setError(null)
    try {
      await action()
      await refresh()
    } catch (reason) {
      setError(String(reason))
    } finally {
      setBusyTaskId(null)
    }
  }

  const title = new Intl.DateTimeFormat(undefined, {
    weekday: 'short',
    month: 'short',
    day: 'numeric',
  }).format(new Date())
  const elapsedSeconds = focus
    ? Math.max(0, Math.floor((now - focus.startTime) / 1000))
    : 0

  return (
    <div className="flex flex-col gap-6">
      <header>
        <p className="text-xs uppercase tracking-wider text-text-muted">
          Today · {localDateKey()}
        </p>
        <h1 className="mt-1 text-2xl font-semibold">{title}</h1>
        <p className="mt-2 text-sm text-text-muted">
          Les tâches issues de tes notes apparaîtront ici quand l’agent sera
          connecté. Tu pourras ensuite démarrer une session pour suivre ton
          temps de concentration.
        </p>
      </header>

      {error && (
        <p
          role="alert"
          className="rounded-md border border-danger/30 px-4 py-3 text-sm text-danger"
        >
          Impossible de mettre à jour les tâches : {error}
        </p>
      )}

      <div className="grid grid-cols-1 gap-6 xl:grid-cols-2">
        <Card title="Session de concentration">
          {focus ? (
            <div className="flex flex-col gap-3">
              <p className="text-sm font-medium">{focus.taskTitle}</p>
              <p
                className="flex items-center gap-2 font-mono text-3xl tabular-nums text-accent"
                aria-live="off"
              >
                <Timer className="h-6 w-6" />
                {formatDuration(elapsedSeconds)}
              </p>
              <p className="text-xs text-text-muted">
                Session démarrée à{' '}
                {new Date(focus.startTime).toLocaleTimeString()}
              </p>
              <button
                type="button"
                disabled={busyTaskId !== null}
                onClick={() =>
                  void runAction(focus.taskId, () =>
                    bridge.tasksPauseFocus(focus.taskId),
                  )
                }
                className="inline-flex w-fit items-center gap-2 rounded-md border border-border px-4 py-2 text-sm text-text-muted hover:text-text disabled:opacity-40"
              >
                <Pause className="h-4 w-4" />
                Mettre en pause
              </button>
            </div>
          ) : (
            <EmptyState
              icon={<Clock3 className="h-8 w-8" />}
              title="Aucune session en cours"
              hint="Choisis une tâche ci-dessous pour démarrer."
            />
          )}
        </Card>

        <Card title="Prochaines tâches">
          {loading ? (
            <p className="py-6 text-sm text-text-muted">Chargement…</p>
          ) : tasks.length === 0 ? (
            <EmptyState
              icon={<Check className="h-8 w-8" />}
              title="Aucune tâche ouverte"
              hint="L’analyse IA des notes n’est pas encore branchée ; les tâches s’afficheront ici une fois générées."
            />
          ) : (
            <ul className="flex flex-col gap-3">
              {tasks.map((task) => {
                const isFocused = focus?.taskId === task.id
                const isBusy = busyTaskId === task.id
                return (
                  <li
                    key={task.id}
                    className="rounded-md border border-border bg-surface-2 px-4 py-3"
                  >
                    <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
                      <div className="min-w-0">
                        <p className="font-medium">{task.title}</p>
                        {task.description && (
                          <p className="mt-1 whitespace-pre-wrap text-sm text-text-muted">
                            {task.description}
                          </p>
                        )}
                        <p className="mt-2 text-xs text-text-muted">
                          {task.status === 'IN_PROGRESS'
                            ? 'En cours'
                            : task.status === 'PAUSED'
                              ? 'En pause'
                              : `${task.estimatedMinutes} min estimées`}
                          {task.actualMinutes > 0 &&
                            ` · ${task.actualMinutes} min enregistrées`}
                        </p>
                      </div>
                      <div className="flex shrink-0 gap-2">
                        {isFocused ? (
                          <button
                            type="button"
                            disabled
                            className="inline-flex items-center gap-2 rounded-md border border-border px-3 py-1.5 text-xs text-text-muted"
                          >
                            <Pause className="h-3.5 w-3.5" />
                            En cours
                          </button>
                        ) : (
                          <button
                            type="button"
                            disabled={focus !== null || busyTaskId !== null}
                            onClick={() =>
                              void runAction(task.id, () =>
                                bridge.tasksStartFocus(task.id),
                              )
                            }
                            className="inline-flex items-center gap-2 rounded-md bg-accent px-3 py-1.5 text-xs font-medium text-white hover:bg-accent/90 disabled:opacity-40"
                          >
                            <Play className="h-3.5 w-3.5" />
                            Démarrer
                          </button>
                        )}
                        <button
                          type="button"
                          disabled={isBusy || busyTaskId !== null}
                          onClick={() =>
                            void runAction(task.id, () =>
                              bridge.tasksComplete(task.id),
                            )
                          }
                          className="inline-flex items-center gap-2 rounded-md border border-border px-3 py-1.5 text-xs text-text-muted hover:text-text disabled:opacity-40"
                        >
                          <Check className="h-3.5 w-3.5" />
                          Terminer
                        </button>
                      </div>
                    </div>
                  </li>
                )
              })}
            </ul>
          )}
        </Card>
      </div>
    </div>
  )
}
