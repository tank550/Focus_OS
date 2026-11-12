import { Clock3 } from 'lucide-react'
import { Card } from '../../components/ui/Card'
import { EmptyState } from '../../components/ui/EmptyState'
import { localDateKey } from '../../lib/date'

/**
 * Today — the primary execution surface.
 * Phase 1 renders the hybrid-plan scaffold; Phase 3 fills the panels with
 * the deterministic scheduler's output (blocks + flexible queue).
 */
export function TodayScreen() {
  const title = new Intl.DateTimeFormat(undefined, {
    weekday: 'short',
    month: 'short',
    day: 'numeric',
  }).format(new Date())

  return (
    <div className="flex flex-col gap-6">
      <header>
        <p className="text-xs uppercase tracking-wider text-text-muted">
          Today · {localDateKey()}
        </p>
        <h1 className="mt-1 text-2xl font-semibold">{title}</h1>
        <p className="mt-2 text-sm text-text-muted">
          Hybrid planning (time blocks + flexible queue) arrives with the
          deterministic scheduler in Phase 3.
        </p>
      </header>

      <div className="grid grid-cols-1 gap-6 xl:grid-cols-2">
        <Card title="Time-blocked timeline">
          <EmptyState
            icon={<Clock3 className="h-8 w-8" />}
            title="No plan for today"
            hint="Deep work sessions, fixed commitments, breaks and buffer windows will render here as scheduled blocks."
          />
        </Card>
        <Card title="Flexible queue">
          <EmptyState
            icon={<Clock3 className="h-8 w-8" />}
            title="Queue is empty"
            hint="Smaller tasks land here as priority-scored items that fit whatever free gaps the day leaves open."
          />
        </Card>
      </div>
    </div>
  )
}