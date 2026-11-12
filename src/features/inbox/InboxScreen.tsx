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
          Write freely. Each entry stays intact as a note; AI can later suggest
          how to organize it into tasks, projects, ideas, and more.
        </p>
      </header>

      <Card title="Write it all down">
        <div className="flex flex-col gap-3">
          <textarea
            data-testid="inbox-input"
            aria-label="Brain dump text"
            rows={8}
            className="w-full resize-y rounded-md border border-border bg-surface-2 px-4 py-3 text-sm leading-6 outline-none transition-colors placeholder:text-text-muted/60 focus:border-accent"
            placeholder={
              inTauri
                ? 'Write everything on your mind — projects, tasks, ideas, worries, unfinished thoughts…'
                : 'Browser mode: capturing requires the Tauri shell (npm run tauri dev).'
            }
            value={text}
            disabled={!inTauri}
            onChange={(e) => setText(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
                e.preventDefault()
                void handleCapture()
              }
            }}
          />
          <div className="flex flex-wrap items-center justify-between gap-3">
            <p className="text-xs text-text-muted">
              Your original text is kept unchanged. Press Ctrl+Enter to save.
            </p>
            <button
              data-testid="inbox-capture-button"
              className="rounded-md bg-accent px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-accent/90 disabled:cursor-not-allowed disabled:opacity-40"
              onClick={() => void handleCapture()}
              disabled={!inTauri || saving || !text.trim()}
            >
              {saving ? 'Saving…' : 'Save brain dump'}
            </button>
          </div>
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
                  <div className="mt-3 flex items-center justify-between gap-3 text-xs text-text-muted">
                    <span>
                      {item.kind === 'UNKNOWN' ? 'Unsorted note' : item.kind}
                    </span>
                    <time dateTime={new Date(item.capturedAt).toISOString()}>
                      {new Date(item.capturedAt).toLocaleString()}
                    </time>
                  </div>
                </li>
              ))}
            </ul>
          )}
        </div>
      </Card>
    </div>
  )
}