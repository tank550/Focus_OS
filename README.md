# Focus OS

AI-powered adaptive personal operating system — local-first, hybrid scheduling,
adaptive autonomy. Turns scattered thoughts and ambitions into consistent,
realistic daily execution.

> **Status:** Phases 2–3 — in progress. Inbox, project management, a visible
> goal Sidecar, and focus sessions are implemented. The local daily planner
> prioritizes work, places deep-work tasks into time blocks, preserves a
> workload reserve, and reports overflow and unresolved dependencies. Flexible
> queue optimization, automatic AI organization, and activity observation
> remain.
> Architecture contract: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)

## Stack

- **Tauri 2.x** desktop shell · **React 19 + TypeScript (strict)** · Vite ·
  Tailwind CSS v4 · Zustand · Lucide
- **SQLite** via `rusqlite` (writes only from the Rust core) — WAL, foreign
  keys, versioned migrations
- **NVIDIA NIM** as the structured-output LLM provider — notes are sent only
  after an explicit analysis action; the model never mutates application state

## Layout

```
docs/                    architecture contract + DDL (source of truth)
crates/focus-domain/     pure-Rust domain model (no Tauri/SQLite deps)
src-tauri/               Rust core: commands, services, db, migrations
src/                     React presentation and explicit AI analysis controls
```

## Development

Prerequisites: Node ≥ 20, Rust (stable), platform webview2.

```bash
npm install
npm run tauri dev        # full desktop app
npm run dev              # frontend only (browser mode, writes disabled)
```

## Gates

```bash
npm run typecheck        # tsc --noEmit (strict)
npm test                 # vitest run
cargo fmt --check
cargo clippy -D warnings
cargo test               # headless domain + SQLite integration tests
```

## Product rules (non-negotiable)

1. SQLite is the source of truth.
2. The LLM never mutates application state.
3. The deterministic scheduler validates all time allocations.
4. Hybrid scheduling is a first-class concept.
5. Hard constraints are never silently violated.
6. Strategic changes require explicit confirmation.
7. Autonomous changes are auditable and reversible.
8. The app stays fully functional when NIM is offline; Discord is optional.
9. Nothing is ever deleted silently.
10. Favor meaningful progress over task-completion metrics.

The Inbox can save an analysis proposal from NVIDIA NIM. A user-confirmed,
actionable task proposal can create an audited task; project creation and
ambiguous proposals still require the remaining agent workflow. Configure the
NIM model and API key in Settings. The mock provider is for offline UI/testing
only and cannot create tasks.

Settings can connect GitHub with a fine-grained read-only token, manually list
repositories owned by the account, and inspect a selected repository locally.
After a separate confirmation, the app sends only the displayed derived
summary to NVIDIA NIM for a portfolio assessment and editable CV draft. The
assessment and draft are stored locally; raw GitHub source is never sent.

Today can create a versioned local plan from open tasks. Deep-work tasks receive
clock-time blocks with buffers; other tasks remain in a flexible priority
queue. The planner respects configured working hours and reserve, reports
overflow and unresolved dependencies, preserves a running block on replans,
and does not otherwise modify task status.