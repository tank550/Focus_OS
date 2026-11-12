import { FolderKanban, Inbox } from 'lucide-react'
import { useEffect, useState } from 'react'
import type { FormEvent } from 'react'
import { Link } from 'react-router-dom'
import { Card } from '../../components/ui/Card'
import { EmptyState } from '../../components/ui/EmptyState'
import { useAppStore } from '../../stores/appStore'
import { useProjectsStore } from '../../stores/projectsStore'
import type { Project, ProjectInput, ProjectStatus } from '../../shared/contracts'

const STATUSES: Array<Exclude<ProjectStatus, 'ARCHIVED'>> = [
  'BACKLOG',
  'ACTIVE',
  'ON_HOLD',
  'COMPLETED',
]

const STATUS_LABELS: Record<Exclude<ProjectStatus, 'ARCHIVED'>, string> = {
  BACKLOG: 'Backlog',
  ACTIVE: 'Active',
  ON_HOLD: 'On hold',
  COMPLETED: 'Completed',
}

const statusLabel = (status: ProjectStatus) =>
  status === 'ARCHIVED' ? 'Archived' : STATUS_LABELS[status]

const inputClass =
  'w-full rounded-md border border-border bg-surface-2 px-3 py-2 text-sm outline-none focus:border-accent'

interface ProjectForm {
  name: string
  description: string
  status: ProjectInput['status']
  priority: string
}

const emptyForm: ProjectForm = {
  name: '',
  description: '',
  status: 'BACKLOG' as const,
  priority: '50',
}

export function ProjectsScreen() {
  const inTauri = useAppStore((state) => state.inTauri)
  const {
    projects,
    loaded,
    mutating,
    error,
    load,
    update,
    archive,
  } = useProjectsStore()
  const [form, setForm] = useState<ProjectForm>(emptyForm)
  const [editingId, setEditingId] = useState<string | null>(null)
  const [formOpen, setFormOpen] = useState(false)
  const [pendingArchiveId, setPendingArchiveId] = useState<string | null>(null)

  useEffect(() => {
    void load()
  }, [load])

  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    if (!editingId) return
    const input: ProjectInput = {
      name: form.name,
      description: form.description || null,
      status: form.status,
      priority: Number(form.priority),
    }

    if (await update(editingId, input)) {
      setForm(emptyForm)
      setEditingId(null)
      setFormOpen(false)
    }
  }

  const startEditing = (project: Project) => {
    setPendingArchiveId(null)
    setEditingId(project.id)
    setForm({
      name: project.name,
      description: project.description ?? '',
      status: project.status === 'ARCHIVED' ? 'BACKLOG' : project.status,
      priority: String(project.priority),
    })
    setFormOpen(true)
  }

  const cancelForm = () => {
    setForm(emptyForm)
    setEditingId(null)
    setFormOpen(false)
  }

  const archiveProject = async (id: string) => {
    if (await archive(id)) setPendingArchiveId(null)
  }

  return (
    <div className="flex flex-col gap-6">
      <header className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <p className="text-xs uppercase tracking-wider text-text-muted">
            Projects
          </p>
          <h1 className="mt-1 text-2xl font-semibold">Projects</h1>
          <p className="mt-2 text-sm text-text-muted">
            Give ongoing efforts a home. Tasks and goal links can be added as
            those parts of the workspace are built.
          </p>
        </div>
        <Link
          className="inline-flex items-center gap-2 rounded-md border border-border px-4 py-2 text-sm font-medium text-text-muted hover:text-text"
          to="/inbox"
        >
          <Inbox className="h-4 w-4" />
          Go to brain dump
        </Link>
      </header>

      {!inTauri && (
        <p className="rounded-md border border-border px-4 py-3 text-sm text-text-muted">
          Projects can be managed in the desktop app. Start it with{' '}
          <code>npm run tauri dev</code>.
        </p>
      )}

      {error && (
        <p role="alert" className="rounded-md border border-danger/30 px-4 py-3 text-sm text-danger">
          {error}
        </p>
      )}

      {formOpen && (
        <Card title="Edit project">
          <form className="flex flex-col gap-4" onSubmit={submit}>
            <label className="flex flex-col gap-1.5 text-sm">
              <span>Project name</span>
              <input
                autoFocus
                className={inputClass}
                maxLength={200}
                required
                value={form.name}
                onChange={(event) =>
                  setForm((current) => ({ ...current, name: event.target.value }))
                }
              />
            </label>
            <label className="flex flex-col gap-1.5 text-sm">
              <span>Description</span>
              <textarea
                className={`${inputClass} resize-y leading-6`}
                rows={4}
                value={form.description}
                onChange={(event) =>
                  setForm((current) => ({
                    ...current,
                    description: event.target.value,
                  }))
                }
              />
            </label>
            <div className="grid gap-4 sm:grid-cols-2">
              <label className="flex flex-col gap-1.5 text-sm">
                <span>Status</span>
                <select
                  className={inputClass}
                  value={form.status}
                  onChange={(event) =>
                    setForm((current) => ({
                      ...current,
                      status: event.target.value as ProjectInput['status'],
                    }))
                  }
                >
                  {STATUSES.map((status) => (
                    <option key={status} value={status}>
                      {STATUS_LABELS[status]}
                    </option>
                  ))}
                </select>
              </label>
              <label className="flex flex-col gap-1.5 text-sm">
                <span>Priority (0–100)</span>
                <input
                  className={inputClass}
                  max={100}
                  min={0}
                  type="number"
                  value={form.priority}
                  onChange={(event) =>
                    setForm((current) => ({
                      ...current,
                      priority: event.target.value,
                    }))
                  }
                />
              </label>
            </div>
            <div className="flex justify-end gap-2">
              <button
                className="rounded-md border border-border px-4 py-2 text-sm text-text-muted hover:text-text"
                disabled={mutating}
                onClick={cancelForm}
                type="button"
              >
                Cancel
              </button>
              <button
                className="rounded-md bg-accent px-4 py-2 text-sm font-medium text-white hover:bg-accent/90 disabled:opacity-40"
                disabled={!inTauri || mutating || !form.name.trim()}
                type="submit"
              >
                {mutating ? 'Saving…' : 'Save changes'}
              </button>
            </div>
          </form>
        </Card>
      )}

      {!loaded ? (
        <p className="text-sm text-text-muted">Loading projects…</p>
      ) : projects.length === 0 ? (
        <EmptyState
          icon={<FolderKanban className="h-10 w-10" />}
          title="No projects discovered yet"
          hint="Projects will be created by the AI agent from your brain-dump notes. You will be able to review the resulting projects here."
        />
      ) : (
        <ul className="grid gap-4 lg:grid-cols-2">
          {projects.map((project) => (
            <li key={project.id}>
              <Card
                title={statusLabel(project.status)}
                action={
                  <span className="font-mono text-[11px] text-text-muted">
                    Priority {project.priority}
                  </span>
                }
              >
                <h2 className="text-base font-semibold">{project.name}</h2>
                {project.description ? (
                  <p className="mt-2 whitespace-pre-wrap text-sm leading-6 text-text-muted">
                    {project.description}
                  </p>
                ) : (
                  <p className="mt-2 text-sm italic text-text-muted">
                    No description yet
                  </p>
                )}
                <div className="mt-4 flex flex-wrap items-center justify-between gap-3">
                  <time
                    className="text-xs text-text-muted"
                    dateTime={new Date(project.updatedAt).toISOString()}
                  >
                    Updated {new Date(project.updatedAt).toLocaleDateString()}
                  </time>
                  <div className="flex gap-2">
                    <button
                      className="rounded-md border border-border px-3 py-1.5 text-xs text-text-muted hover:text-text disabled:opacity-40"
                      disabled={mutating}
                      onClick={() => startEditing(project)}
                    >
                      Edit
                    </button>
                    <button
                      className="rounded-md border border-border px-3 py-1.5 text-xs text-danger hover:bg-danger/10 disabled:opacity-40"
                      disabled={mutating}
                      onClick={() => {
                        setFormOpen(false)
                        setPendingArchiveId(project.id)
                      }}
                    >
                      Archive
                    </button>
                  </div>
                </div>
                {pendingArchiveId === project.id && (
                  <div
                    className="mt-3 flex flex-wrap items-center justify-between gap-3 rounded-md border border-danger/30 bg-danger/5 px-3 py-2"
                    role="group"
                    aria-label="Confirm project archive"
                  >
                    <p className="text-xs text-text-muted">
                      Archive this project? Its record and related data will be
                      kept.
                    </p>
                    <div className="flex gap-2">
                      <button
                        className="rounded-md border border-border px-3 py-1.5 text-xs text-text-muted hover:text-text"
                        onClick={() => setPendingArchiveId(null)}
                      >
                        Keep project
                      </button>
                      <button
                        className="rounded-md bg-danger px-3 py-1.5 text-xs font-medium text-white disabled:opacity-40"
                        disabled={mutating}
                        onClick={() => void archiveProject(project.id)}
                      >
                        {mutating ? 'Archiving…' : 'Confirm archive'}
                      </button>
                    </div>
                  </div>
                )}
              </Card>
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
