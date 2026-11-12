# Focus OS

AI-powered adaptive personal operating system — local-first, hybrid scheduling,
adaptive autonomy. Turns scattered thoughts and ambitions into consistent,
realistic daily execution.

> **Status:** Phase 2 (Core domain) — in progress. Inbox, project management,
> a visible goal Sidecar, and basic task focus sessions are implemented.
> Automatic AI organization, scheduling, and system activity observation remain.
> Architecture contract: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)

## Stack

- **Tauri 2.x** desktop shell · **React 19 + TypeScript (strict)** · Vite ·
  Tailwind CSS v4 · Zustand · Lucide
- **SQLite** via `rusqlite` (writes only from the Rust core) — WAL, foreign
  keys, versioned migrations
- **NVIDIA NIM** as the (future) structured-output LLM provider — no state
  mutations from the model layer, ever

## Layout

```
docs/                    architecture contract + DDL (source of truth)
crates/focus-domain/     pure-Rust domain model (no Tauri/SQLite deps)
src-tauri/               Rust core: commands, services, db, migrations
src/                     React presentation + (Phase 4) AI orchestration
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