import { Inbox as InboxIcon } from 'lucide-react'
import { useState } from 'react'
import { Card } from '../../components/ui/Card'
import { EmptyState } from '../../components/ui/EmptyState'
import { useAppStore } from '../../stores/appStore'

/**
 * Inbox — the universal capture mechanism.
 * Phase 1 validates the input flow in the shell only. Phase 2 wires the
 * `inbox_capture` command: a single transactional insert with zero AI.
 */
export function InboxScreen() {
  const [text, setText] = useState('')
  const inTauri = useAppStore((s) => s.inTauri)

  const capture = () => {
    if (!text.trim()) return
    // Phase 2: bridge.capture({ rawText: text.trim() }) — a single insert.
    setText('')
  }

  return (
    <div className="flex flex-col gap-6">
      <header>
        <p className="text-xs uppercase tracking-wider text-text-muted">
          Inbox
        </p>
        <h1 className="mt-1 text-2xl font-semibold">Brain Dump</h1>
        <p className="mt-2 text-sm text-text-muted">
          Capture anything — tasks, ideas, projects, obligations. Saving is
          instant and never requires AI.
        </p>
      </header>

      <Card title="Quick capture">
        <div className="flex gap-3">
          <input
            data-testid="inbox-input"
            className="flex-1 rounded-md border border-border bg-surface-2 px-3 py-2 text-sm outline-none transition-colors placeholder:text-text-muted/60 focus:border-accent"
            placeholder={
              inTauri
                ? 'Capture anything — it saves instantly, no AI required…'
                : 'Browser mode: capturing requires the Tauri shell (npm run tauri dev).'
            }
            value={text}
            disabled={!inTauri}
            onChange={(e) => setText(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') capture()
            }}
          />
          <button
            data-testid="inbox-capture-button"
            className="rounded-md bg-accent px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-accent/90 disabled:cursor-not-allowed disabled:opacity-40"
            onClick={capture}
            disabled={!inTauri}
          >
            Save
          </button>
        </div>
        <div className="mt-4">
          <EmptyState
            icon={<InboxIcon className="h-8 w-8" />}
            title="No captured items"
            hint="Raw text persists immediately. AI classification suggestions arrive asynchronously and are inspectable (Phase 4)."
          />
        </div>
      </Card>
    </div>
  )
}