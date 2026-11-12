import { Outlet } from 'react-router-dom'
import { Sidebar } from '../components/Sidebar'
import { useAppStore } from '../stores/appStore'

export function AppLayout() {
  const { inTauri, health, error, loaded } = useAppStore()

  const status = !loaded
    ? 'Loading…'
    : error
      ? error
      : inTauri
        ? health
          ? `DB ${health.db} · ${health.migrationsApplied} migrations`
          : 'DB n/a'
        : 'Browser mode — data writes disabled'

  return (
    <div className="flex h-full w-full">
      <Sidebar />
      <main className="min-w-0 flex-1 overflow-y-auto">
        <div className="mx-auto flex h-full max-w-6xl flex-col px-8 py-6">
          <div className="mb-4 flex items-center justify-end">
            <span
              data-testid="runtime-badge"
              className={`rounded-full border px-3 py-1 font-mono text-[11px] ${
                error
                  ? 'border-danger/40 text-danger'
                  : 'border-border text-text-muted'
              }`}
            >
              {status}
            </span>
          </div>
          <div className="flex-1 pb-10">
            <Outlet />
          </div>
        </div>
      </main>
    </div>
  )
}