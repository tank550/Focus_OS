import { Target } from 'lucide-react'
import { EmptyState } from '../../components/ui/EmptyState'

export function GoalsScreen() {
  return (
    <div className="flex flex-col gap-6">
      <header>
        <p className="text-xs uppercase tracking-wider text-text-muted">
          Goals
        </p>
        <h1 className="mt-1 text-2xl font-semibold">Quarterly Objectives</h1>
      </header>
      <EmptyState
        icon={<Target className="h-10 w-10" />}
        title="No quarterly goals yet"
        hint="90-day objectives decompose into milestones → projects → tasks. Strategic goals are never mutated autonomously."
      />
    </div>
  )
}