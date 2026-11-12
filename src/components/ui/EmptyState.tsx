import type { ReactNode } from 'react'

export function EmptyState({
  icon,
  title,
  hint,
}: {
  icon: ReactNode
  title: string
  hint?: string
}) {
  return (
    <div className="flex flex-col items-center justify-center gap-2 py-10 text-center">
      <div className="text-text-muted">{icon}</div>
      <p className="text-sm font-medium text-text">{title}</p>
      {hint && (
        <p className="max-w-sm text-xs leading-relaxed text-text-muted">
          {hint}
        </p>
      )}
    </div>
  )
}