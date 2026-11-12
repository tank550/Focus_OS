import { CalendarCheck2 } from 'lucide-react'
import { EmptyState } from '../../components/ui/EmptyState'

export function AnalyticsScreen() {
  return (
    <div className="flex flex-col gap-6">
      <header>
        <p className="text-xs uppercase tracking-wider text-text-muted">
          Analytics
        </p>
        <h1 className="mt-1 text-2xl font-semibold">Analytics</h1>
      </header>
      <EmptyState
        icon={<CalendarCheck2 className="h-10 w-10" />}
        title="Nothing to analyze yet"
        hint="Time distribution, planning accuracy, goal progress and the autonomy audit trail arrive with execution data (Phase 6+)."
      />
    </div>
  )
}