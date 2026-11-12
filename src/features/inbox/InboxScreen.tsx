import { Inbox as InboxIcon } from 'lucide-react'
import { useEffect, useState } from 'react'
import { Card } from '../../components/ui/Card'
import { EmptyState } from '../../components/ui/EmptyState'
import { useAppStore } from '../../stores/appStore'
import { useInboxStore } from '../../stores/inboxStore'

export function InboxScreen() {
  const [text, setText] = useState('')
  const inTauri = useAppStore((s) => s.inTauri)
  const { items, loaded, saving, error, load, capture } = useInboxStore()

  useEffect(() => {
    void load()
  }, [load])

  const handleCapture = async () => {
    if (await capture(text)) setText('')
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
              if (e.key === 'Enter' && !e.shiftKey) {
                e.preventDefault()
                void handleCapture()
              }
            }}
          />
          <button
            data-testid="inbox-capture-button"
            className="rounded-md bg-accent px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-accent/90 disabled:cursor-not-allowed disabled:opacity-40"
            onClick={() => void handleCapture()}
            disabled={!inTauri || saving || !text.trim()}
          >
            {saving ? 'Saving…' : 'Save'}
          </button>
        </div>
        {error && (
          <p role="alert" className="mt-3 text-sm text-danger">
            {error}
          </p>
        )}
        <div className="mt-4">
          {!loaded ? (
            <p className="text-sm text-text-muted">Loading inbox…</p>
          ) : items.length === 0 ? (
            <EmptyState
              icon={<InboxIcon className="h-8 w-8" />}
              title="No captured items"
              hint="Captured thoughts are saved locally and can be organized later."
            />
          ) : (
            <ul className="flex flex-col gap-3">
              {items.map((item) => (
                <li
                  key={item.id}
                  className="rounded-md border border-border bg-surface-2 px-4 py-3"
                >
                  <p className="whitespace-pre-wrap text-sm">{item.rawText}</p>
                  <p className="mt-2 text-xs text-text-muted">
                    {item.kind} · {new Date(item.capturedAt).toLocaleString()}
                  </p>
                </li>
              ))}
            </ul>
          )}
        </div>
      </Card>
    </div>
  )
}