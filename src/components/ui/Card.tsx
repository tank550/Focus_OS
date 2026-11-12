import type { ReactNode } from 'react'

export function Card({
  title,
  action,
  children,
}: {
  title?: string
  action?: ReactNode
  children: ReactNode
}) {
  return (
    <section className="rounded-xl border border-border bg-surface">
      {title && (
        <header className="flex items-center justify-between border-b border-border px-4 py-3">
          <h2 className="text-xs font-semibold uppercase tracking-wider text-text-muted">
            {title}
          </h2>
          {action}
        </header>
      )}
      <div className="p-4">{children}</div>
    </section>
  )
}