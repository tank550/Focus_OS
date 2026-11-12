import { Activity, Target } from 'lucide-react'
import { useEffect, useState } from 'react'
import type { FormEvent } from 'react'
import { Card } from '../../components/ui/Card'
import { EmptyState } from '../../components/ui/EmptyState'
import { useAppStore } from '../../stores/appStore'
import { useGoalsStore } from '../../stores/goalsStore'

const fieldClass =
  'w-full rounded-md border border-border bg-surface-2 px-3 py-2 text-sm outline-none focus:border-accent'

export function GoalsScreen() {
  const inTauri = useAppStore((state) => state.inTauri)
  const {
    goals,
    sidecarByGoal,
    loadingSidecar,
    loaded,
    saving,
    error,
    load,
    loadSidecar,
    create,
    addCheckIn,
  } = useGoalsStore()
  const [formOpen, setFormOpen] = useState(false)
  const [title, setTitle] = useState('')
  const [description, setDescription] = useState('')
  const [outcome, setOutcome] = useState('')
  const [criteria, setCriteria] = useState('')
  const [checkIns, setCheckIns] = useState<Record<string, string>>({})

  useEffect(() => {
    void load()
  }, [load])

  useEffect(() => {
    for (const goal of goals) void loadSidecar(goal.id)
  }, [goals, loadSidecar])

  const submitGoal = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    const succeeded = await create({
      title,
      description: description || null,
      expectedOutcome: outcome || null,
      successCriteria: criteria
        .split(/\r?\n/)
        .map((criterion) => criterion.trim())
        .filter(Boolean),
    })
    if (succeeded) {
      setTitle('')
      setDescription('')
      setOutcome('')
      setCriteria('')
      setFormOpen(false)
    }
  }

  const submitCheckIn = async (goalId: string) => {
    const content = checkIns[goalId] ?? ''
    if (await addCheckIn(goalId, content)) {
      setCheckIns((current) => ({ ...current, [goalId]: '' }))
    }
  }

  return (
    <div className="flex flex-col gap-6">
      <header className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <p className="text-xs uppercase tracking-wider text-text-muted">
            Goals
          </p>
          <h1 className="mt-1 text-2xl font-semibold">Quarterly Objectives</h1>
          <p className="mt-2 max-w-2xl text-sm text-text-muted">
            Goals are yours to define. The Sidecar keeps observed evidence
            alongside them so you can compare intention with what actually
            happened.
          </p>
        </div>
        <button
          className="rounded-md bg-accent px-4 py-2 text-sm font-medium text-white hover:bg-accent/90 disabled:opacity-40"
          disabled={!inTauri || saving}
          onClick={() => setFormOpen((open) => !open)}
        >
          {formOpen ? 'Close' : 'Set an objective'}
        </button>
      </header>

      <Card title="Sidecar data sources">
        <p className="mb-3 text-sm text-text-muted">
          The Sidecar is separate from official goal progress. These sources will
          be opt-in and locally visible; system and integration tracking are not
          active yet.
        </p>
        <div className="grid gap-3 md:grid-cols-3">
          {[
            ['Focus OS', 'Tasks and focus sessions'],
            ['Integrations', 'Calendars and connected tools'],
            ['System activity', 'App names, window titles, and domains'],
          ].map(([source, detail]) => (
            <div
              key={source}
              className="rounded-md border border-border bg-surface-2 px-3 py-2"
            >
              <p className="text-sm font-medium">{source}</p>
              <p className="mt-1 text-xs text-text-muted">{detail}</p>
              <p className="mt-2 text-[10px] uppercase tracking-wider text-text-muted">
                Not connected
              </p>
            </div>
          ))}
        </div>
      </Card>

      {!inTauri && (
        <p className="rounded-md border border-border px-4 py-3 text-sm text-text-muted">
          Goals and Sidecar check-ins can be managed in the desktop app. Start
          it with <code>npm run tauri dev</code>.
        </p>
      )}

      {error && (
        <p
          role="alert"
          className="rounded-md border border-danger/30 px-4 py-3 text-sm text-danger"
        >
          {error}
        </p>
      )}

      {formOpen && (
        <Card title="Define a strategic objective">
          <form className="flex flex-col gap-4" onSubmit={submitGoal}>
            <label className="flex flex-col gap-1.5 text-sm">
              <span>Objective</span>
              <input
                autoFocus
                className={fieldClass}
                required
                value={title}
                onChange={(event) => setTitle(event.target.value)}
              />
            </label>
            <label className="flex flex-col gap-1.5 text-sm">
              <span>Why does it matter?</span>
              <textarea
                className={`${fieldClass} resize-y`}
                rows={2}
                value={description}
                onChange={(event) => setDescription(event.target.value)}
              />
            </label>
            <label className="flex flex-col gap-1.5 text-sm">
              <span>What outcome would count as success?</span>
              <textarea
                className={`${fieldClass} resize-y`}
                rows={2}
                value={outcome}
                onChange={(event) => setOutcome(event.target.value)}
              />
            </label>
            <label className="flex flex-col gap-1.5 text-sm">
              <span>Success criteria (one per line)</span>
              <textarea
                className={`${fieldClass} resize-y`}
                rows={3}
                value={criteria}
                onChange={(event) => setCriteria(event.target.value)}
              />
            </label>
            <div className="flex justify-end">
              <button
                className="rounded-md bg-accent px-4 py-2 text-sm font-medium text-white hover:bg-accent/90 disabled:opacity-40"
                disabled={!inTauri || saving || !title.trim()}
                type="submit"
              >
                {saving ? 'Saving…' : 'Save objective'}
              </button>
            </div>
          </form>
        </Card>
      )}

      {!loaded ? (
        <p className="text-sm text-text-muted">Loading objectives…</p>
      ) : goals.length === 0 ? (
        <EmptyState
          icon={<Target className="h-10 w-10" />}
          title="No quarterly goals yet"
          hint="Define the strategic direction here. The AI can later derive projects and tasks from your notes without changing these goals."
        />
      ) : (
        <ul className="flex flex-col gap-5">
          {goals.map((goal) => {
            const entries = sidecarByGoal[goal.id] ?? []
            return (
              <li key={goal.id}>
                <div className="grid gap-4 xl:grid-cols-2">
                  <Card title="Objective">
                    <div className="flex items-start justify-between gap-4">
                      <h2 className="text-base font-semibold">{goal.title}</h2>
                      <span className="shrink-0 rounded-full border border-border px-2.5 py-1 text-[10px] uppercase tracking-wider text-text-muted">
                        {goal.status}
                      </span>
                    </div>
                    {goal.description && (
                      <p className="mt-3 whitespace-pre-wrap text-sm text-text-muted">
                        {goal.description}
                      </p>
                    )}
                    {goal.expectedOutcome && (
                      <div className="mt-4">
                        <p className="text-xs font-semibold uppercase tracking-wider text-text-muted">
                          Expected outcome
                        </p>
                        <p className="mt-1 text-sm">{goal.expectedOutcome}</p>
                      </div>
                    )}
                    {goal.successCriteria.length > 0 && (
                      <div className="mt-4">
                        <p className="text-xs font-semibold uppercase tracking-wider text-text-muted">
                          Success criteria
                        </p>
                        <ul className="mt-2 list-inside list-disc space-y-1 text-sm text-text-muted">
                          {goal.successCriteria.map((criterion, index) => (
                            <li key={`${goal.id}-${index}`}>{criterion}</li>
                          ))}
                        </ul>
                      </div>
                    )}
                    <p className="mt-4 text-xs text-text-muted">
                      Official progress: {Math.round(goal.progress * 100)}%
                      <span className="ml-2">
                        (Sidecar observations do not change this value.)
                      </span>
                    </p>
                  </Card>

                  <Card
                    title="Sidecar · observed evidence"
                    action={
                      <span className="font-mono text-[11px] text-text-muted">
                        {entries.length} entries
                      </span>
                    }
                  >
                    <div className="flex items-start gap-2 rounded-md border border-border bg-surface-2 px-3 py-2">
                      <Activity className="mt-0.5 h-4 w-4 shrink-0 text-accent" />
                      <p className="text-xs leading-relaxed text-text-muted">
                        This evidence stream is independent of the objective.
                        AI-generated assessments will be labeled and reviewable;
                        none are generated until an AI provider is configured.
                      </p>
                    </div>
                    <label className="mt-4 flex flex-col gap-1.5 text-sm">
                      <span>Check in: what happened toward this goal?</span>
                      <textarea
                        className={`${fieldClass} resize-y`}
                        rows={3}
                        value={checkIns[goal.id] ?? ''}
                        onChange={(event) =>
                          setCheckIns((current) => ({
                            ...current,
                            [goal.id]: event.target.value,
                          }))
                        }
                      />
                    </label>
                    <div className="mt-2 flex justify-end">
                      <button
                        className="rounded-md border border-border px-3 py-1.5 text-xs font-medium text-text-muted hover:text-text disabled:opacity-40"
                        disabled={
                          !inTauri || saving || !(checkIns[goal.id] ?? '').trim()
                        }
                        onClick={() => void submitCheckIn(goal.id)}
                      >
                        Add check-in
                      </button>
                    </div>
                    <div className="mt-4">
                      {loadingSidecar[goal.id] ? (
                        <p className="text-xs text-text-muted">
                          Loading Sidecar…
                        </p>
                      ) : entries.length === 0 ? (
                        <p className="text-xs text-text-muted">
                          No observations yet.
                        </p>
                      ) : (
                        <ul className="flex max-h-72 flex-col gap-2 overflow-y-auto">
                          {entries.map((entry) => (
                            <li
                              className="rounded-md border border-border px-3 py-2"
                              key={entry.id}
                            >
                              <p className="whitespace-pre-wrap text-sm">
                                {entry.content}
                              </p>
                              {(entry.applicationName ||
                                entry.windowTitle ||
                                entry.domain) && (
                                <p className="mt-2 break-words text-xs text-text-muted">
                                  {[entry.applicationName, entry.windowTitle, entry.domain]
                                    .filter(Boolean)
                                    .join(' · ')}
                                </p>
                              )}
                              <div className="mt-2 flex justify-between gap-2 text-[10px] uppercase tracking-wider text-text-muted">
                                <span>
                                  {entry.source} · {entry.entryType}
                                  {entry.reviewStatus === 'PENDING'
                                    ? ' · Needs review'
                                    : ''}
                                </span>
                                <time
                                  dateTime={new Date(entry.createdAt).toISOString()}
                                >
                                  {new Date(entry.createdAt).toLocaleDateString()}
                                </time>
                              </div>
                            </li>
                          ))}
                        </ul>
                      )}
                    </div>
                  </Card>
                </div>
              </li>
            )
          })}
        </ul>
      )}
    </div>
  )
}
