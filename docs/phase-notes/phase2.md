# Phase 2 — Core domain (status notes)

Goal: domain vocabulary, SQLite repositories, Inbox/Project/Goal/Task
services, goal Sidecar, and typed IPC commands.

## Exit criteria / checklist

- [x] Domain vocabulary with SQL-contract tests (`focus-domain`)
- [x] Inbox capture persists synchronously, with no AI in the write path
- [x] Inbox analysis proposals are persisted and applied only after confirmation
- [x] Project list/update/archive round-trips through IPC
- [x] Goal creation and Sidecar check-ins, separate from official goal state
- [x] Task creation from the note agent + focus sessions
- [x] Task dependency blockers are read by the planner
- [x] Integration tests for migrations, constraints, and transactions
- [ ] Full task CRUD (edit title/estimate/priority/deadline, soft-delete)
- [ ] Dependency CRUD through IPC
- [ ] Milestone CRUD and goal status transitions
- [ ] Project detail view (`/projects/:id`)
- [ ] `tauri-specta` codegen (contracts are still hand-written)

## Delivered in this phase

- `crates/focus-domain`: IDs, status enums, value objects, typed errors.
- Repositories: inbox, projects, goals (+ Sidecar), tasks (+ sessions),
  planner, portfolio, settings, autonomy decisions.
- Application services enforce the transaction boundary; the webview has no
  SQL capability.
- Note agent: NIM/mock analysis, stored proposal, idempotent
  confirmation-then-create for a task or a project with an optional first task,
  audited in `autonomy_decisions`.
- 23 tables after migrations v0001–v0003.

## Known gaps

- The Projects screen is read/manage only; tasks and goal links are not yet
  editable from a project detail screen.
- Task transitions are limited to start/pause/complete focus actions.
- Goal progress is stored but not derived from milestones.
- The note agent accepts only task/project creation; other analysis kinds remain
  suggestions.

## Environment notes

- Frontend toolchain pinned to Vite 6 / Vitest 3 with Node 22.
- Rust builds headlessly on Linux for the test suite; the committed baseline
  targets Tauri 2 / Rust 1.90.
