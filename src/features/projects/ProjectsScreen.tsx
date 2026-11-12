import { FolderKanban } from 'lucide-react'
import { EmptyState } from '../../components/ui/EmptyState'

export function ProjectsScreen() {
  return (
    <div className="flex flex-col gap-6">
      <header>
        <p className="text-xs uppercase tracking-wider text-text-muted">
          Projects
        </p>
        <h1 className="mt-1 text-2xl font-semibold">Projects</h1>
      </header>
      <EmptyState
        icon={<FolderKanban className="h-10 w-10" />}
        title="No projects yet"
        hint="Projects with status, priority, workload and progress enter the model in Phase 2."
      />
    </div>
  )
}