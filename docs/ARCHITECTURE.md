# Focus OS — Technical Architecture

> **Version:** 1.0 (draft for approval) · **Date:** 2026-11-10
> **Status:** AWAITING APPROVAL — no implementation code has been generated yet.
> **Companion artifact:** `docs/SQLITE_SCHEMA.sql` (complete initial DDL).

---

## 0. Executive Summary

Focus OS is a local-first, AI-powered desktop application for personal
productivity that behaves like an AI chief of staff. It is not a todo list: it
combines a **time-blocked deep-work schedule** with a **flexible,
priority-scored task queue** (hybrid scheduling) and an **adaptive autonomy
engine** that lets an LLM propose actions while deterministic code decides
whether those actions run automatically, are surfaced for approval, or require
explicit confirmation.

The system is architected as a **Rust-native core** (domain model, persistence,
scheduler, autonomy policy, native integrations) with a **TypeScript
presentation layer** (React UI, AI orchestration, response validation).
SQLite is the single source of truth; the LLM is a bounded advisory component
that **never mutates application state directly**. Every mutation flows through
application services inside a transaction boundary, every autonomous mutation is
audited and reversible, and hard scheduling constraints are validated by pure,
unit-tested functions — never by the model.

This document is the source-of-truth contract for the phased implementation in
§12. It pins down the data model, the hybrid scheduling algorithm, the autonomy
decision model, the NVIDIA NIM integration strategy, the Tauri/React shell, and
the testing gates. Publication of this document does not start implementation;
Phase 1 begins only after approval.

---

## 1. Product Principles → Enforcement Mapping

Each non-negotiable product rule is enforced by a concrete architectural
mechanism, never by programmer convention:

| Product rule | Enforcing mechanism |
|---|---|
| SQLite is the source of truth | All state mutations persist in SQLite via Rust application services within a transaction. No in-memory-only business state. The frontend is a read-model projection + commander. |
| LLM never mutates state directly | NIM HTTP client runs in Rust. LLM output is Zod-validated into **proposed operations** (`Operation[]`), which are executed only by Rust application services. The webview has no SQL capability. |
| Deterministic scheduler validates all allocations | `SchedulerService` is authoritative over `scheduled_blocks` / `flexible_queue_items`. `validate_schedule()` is pure and unit-tested. AI-suggested schedules are fed through the same validator. |
| Hybrid scheduling is a first-class concept | Two disjoint scheduling artifacts — `scheduled_blocks` (time-anchored) and `flexible_queue_items` (priority-ordered). A task appears in exactly one of them per daily plan. |
| Flexible tasks must not become fake appointments | The queue is a planning structure, not time slots; blocks are created only for deep work, sessions, commitments, breaks, buffers, unavailability. |
| Active tasks not interrupted by minor replanning | Replanning freezes the currently `ACTIVE` block and operates on the horizon after `now + active_window`. |
| Hard constraints never silently violated | `validate_schedule()` runs before any plan persists; violations produce a typed error and the offending operation is rejected. |
| Strategic changes require explicit confirmation | Autonomy policy maps goal/milestone/deadline/fixed-commitment mutations to `REQUIRE_CONFIRMATION`; an approval dialog blocks execution. |
| Autonomous changes auditable + reversible | Append-only `autonomy_decisions` + `schedule_events` carrying `undo_payload`; revert rehydrates prior state. |
| Do not overload the day | Workload model caps planned minutes, deep-work-block count, consecutive focus time, and reserves buffer *before* placement. Overflow is reported explicitly, never crammed. |
| Capture without interrupting | Quick-capture overlay writes one `inbox_items` row in a single transaction; the running focus session is untouched. |
| Functional when NIM is unavailable | `LlmProvider` trait with `MockProvider`; classification/planning degrade to deterministic defaults + reuse of last-good history. |
| Discord optional | Feature flag + keyring-stored URL; failures are logged and swallowed (fire-and-forget with bounded retry). |
| No silent deletion | Soft deletion (`archived` / `deleted_at`) everywhere; `schedule_events` retains undo payloads; purge requires an explicit maintenance command. |

### 1.1 The adaptive-autonomy dichotomy

The two product decisions — **hybrid scheduling** and **adaptive autonomy** —
are reflected in the component topology itself:

- **Semantic intelligence** (classify, decompose, explain, narrate) is owned by
  the LLM layer and is *advisory*.
- **Determinism** (scores, placements, constraints, autonomy classification,
  transactions) is owned by Rust and is *authoritative*.

The interface between them is the validated `ProposedOperation` — a small,
closed algebraic data type. It is the only shape in which AI output may enter
the persistence boundary. This guarantees that "the AI recommends, the system
decides" holds structurally, not just by convention.

---

## 2. System Architecture Overview

```
┌───────────────────────────────────────────────────────────────────────────┐
│                      FRONTEND (Tauri WebView)                             │
│                                                                           │
│  React 19 + TS strict — screens (today / inbox / projects / goals / …)    │
│  Zustand stores — typed DTOs — AI Orchestrators (context assembly,        │
│  prompts, Zod validation → ProposedOperation[])                           │
└──────────────▲──────────────────────────────────────┬─────────────────────┘
               │ invoke() typed commands (tauri-specta)│ push events
┌──────────────┴──────────────────────────────────────▼─────────────────────┐
│                            RUST CORE (src-tauri)                          │
│                                                                           │
│  ┌────────────────┐  ┌───────────────────┐  ┌──────────────────────────┐  │
│  │ ISOLATION LAYER │  │ APPLICATION SVC   │  │ AI CLIENT                 │  │
│  │ typed commands  │──▶│ use cases,        │  │ LlmProvider trait         │  │
│  │ input validation│  │ transaction       │  │ NimProvider (reqwest)     │  │
│  └────────────────┘  │ boundary           │  │ retries / timeouts / SSE  │  │
│  ┌───────────────▲───┤                    │  └──────────▲───────────────┘  │
│  │ DOMAIN (pure) │   └───────┬────────────┘             │  /v1/chat/completions
│  │ entities      │           │                          │
│  │ value objects │   ┌───────▼────────────┐  ┌──────────┴───────────────┐  │
│  │ invariants    │   │ SCHEDULER ENGINE   │  │ notifications·discord·     │  │
│  │ autonomy      │◀──│ scoring constraints│  │ tray·timer·keyring·cron    │  │
│  │ policy        │   │ allocation queue   │  │ (tokio background tasks)   │  │
│  └───────────────┘   └───────┬────────────┘  └──────────▲───────────────┘  │
│  ┌───────────────────────────▼──────────────────────────▼───────────────┐  │
│  │ PERSISTENCE — Db (rusqlite·WAL·pool) · Migrations · Repositories      │  │
│  └───────────────────────────▲───────────────────────────────────────────┘  │
└──────────────────────────────┼──────────────────────────────────────────────┘
                               │ SQLite in app-data dir (focus-os.db)
                               ▼
```

### 2.1 Control-flow rules

1. **UI → Isolation → Application Service → Repository → SQLite.** The frontend
   cannot reach SQLite — the webview has no SQL capability (`tauri-plugin-sql`
   is deliberately *not* installed).
2. **LLM → context assembly → NIM → Zod validation → `ProposedOperation[]` →
   Autonomy Policy → Application Service.** When the autonomy level is
   `PROPOSE` or `REQUIRE_CONFIRMATION` the service persists an
   `approval_requests` row and returns it; nothing else is mutated.
3. **Scheduler reads, validates, commits.** Time allocations always pass through
   `SchedulerService::commit_plan`, which runs `validate_schedule()` and writes
   plan + blocks + queue + audit event in a single transaction.
4. **LLM never persists.** Even validated operations are executed solely by
   Rust services. The AI layer owns prompts and schemas; Rust owns writes.

---

## 3. Technology Decisions

| Area | Choice | Rationale | Rejected alternative |
|---|---|---|---|
| Shell | Tauri 2.x | ~10 MB binaries, Rust native core, no JVM/Node runtime; capabilities map cleanly to our native needs | Electron (memory footprint, weaker native boundary) |
| UI | React 19 + Vite + TS strict | Mature, typed, fast HMR; strict mode enforced by `tsconfig` | Svelte (excellent, but team ergonomics + ecosystem) |
| Styling | Tailwind CSS v4 + Lucide | Design-system velocity, dark-first theming via CSS variables | hand-rolled CSS (slower), MUI (too heavy) |
| DB | SQLite (`rusqlite`) | Zero-ops local source of truth; FKs, CHECKs, WAL | `tauri-plugin-sql` (generic, no domain control; bypasses our service layer) |
| Migrations | Embedded SQL migrations run by the Rust core at startup | Versioned, transactional (each migration in its own transaction), testable | `refinery` (extra dep; wrapper overhead is small but unnecessary) |
| Rust local state | `tokio` (async services) + `r2d2_sqlite` pool | Async timers/notifications; pooled writes; WAL single-writer | naive blocking connection shared across tasks |
| Native plugins | global-shortcut, notification, single-instance, autostart, window-state, opener, log | Official plugin ecosystem verified in §research | `tauri-plugin-sql`, `tauri-plugin-http` — deliberately excluded |
| Credentials | `tauri-plugin-keyring` (keyring-core) | Windows Credential Manager / macOS Keychain / Linux Secret Service — exactly the spec requirement | `tauri-plugin-stronghold` (encrypted file vault, not OS keychain — fallback if keyring is unavailable) |
| HTTP/AI | Rust `reqwest` + SSE parser | CORS-free, retries/timeouts testable with wiremock; provider abstraction for future LLMs | `fetch` in webview (CORS, secrets in webview memory, no retry policy) |
| IPC typing | `tauri-specta` (Rust → TS codegen) | Compile-time sync of command DTOs; eliminates drift | hand-written `contracts.ts` (drift risk) — fallback if specta pegs to an unusable Tauri version |
| State | Zustand | Small, observable, no boilerplate | Redux Toolkit (heavier than needed) |
| Validation | Zod (AI outputs) + serde (command inputs) | Double boundary: runtime validation of unreliable input at both edges | type-only validation (no runtime guarantees against LLM output) |
| Tests | `cargo test` (unit/integration) + `vitest` (TS) + `wiremock` (NIM) | Deterministic engine tests run headless in CI without a webview | Playwright-only (flake-prone, slow for engine logic) |

### 3.1 Why the domain lives in Rust

The spec requires the scheduler, autonomy policy, and constraints to be
deterministic and independently testable, and SQLite to be the source of truth.
Implementing the domain in Rust co-locates invariants with the transaction
boundary: a scheduler rule cannot be bypassed by a future UI path because *every*
write goes through the same Rust services. TypeScript is excellent for
presentation and AI orchestration; it is deliberately denied write access.

**Trade-off (accepted):** higher initial cost (two language layers, codegen for
contracts) in exchange for a single authoritative enforcement point. The
alternative — domain duplicate in both layers — guarantees eventual divergence.

### 3.2 Library policy

Add a dependency only when it earns its place: no analytics SDKs, no runtime
ORM, no state-sync frameworks, no UI kit. Every crate must be broadly
maintained and already present in the current ecosystem state.

---

## 4. Repository / Module Layout

```
d:\Project\daily_list\
├─ docs\                         # architecture, schema, phase notes
├─ src\                          # TypeScript frontend (presentation + AI orchestration)
│  ├─ app\                       # providers, router, theme, error boundary
│  ├─ shared\                    # contracts.ts (codegen), id, dates, string utils
│  ├─ domain\                    # TS read-model types + Zod schemas for AI output
│  ├─ ai\                        # providers (nim/mock), context selectors, token budgets
│  │  ├─ providers\              # LlmProvider interface, NimProvider, MockProvider
│  │  ├─ schemas\                # zod schemas: brainDump, classifier, goalPlan, dailyPlan…
│  │  └─ orchestrators\          # BrainDumpAnalyzer, TaskClassifier, ProjectAnalyzer,
│  │                             # GoalPlanner, DailyPlanner, ScheduleOptimizer,
│  │                             # ScheduleChangeExplainer, DailyReviewAnalyzer,
│  │                             # QuarterlyReviewAnalyzer
│  ├─ features\                  # today\ inbox\ projects\ goals\ focus\ analytics\
│  │                             # settings\
│  ├─ components\                # ui primitives (button, card, dialog, timeline…)
│  ├─ stores\                    # Zustand slices per feature
│  └─ lib\                       # bridge (typed invoke wrappers), event bus
│
├─ src-tauri\
│  ├─ src\
│  │  ├─ commands\               # typed #[command] fns (thin, validation only)
│  │  ├─ domain\                 # entities, value-objects, policies, invariants
│  │  ├─ application\            # services: Inbox, Project, Goal, Scheduler, Task,
│  │  │                          # Autonomy, Review, Settings, Session
│  │  ├─ infrastructure\
│  │  │  ├─ db\                  # pool, migrations, migrations/*.sql
│  │  │  ├─ repositories\        # sqlite implementations
│  │  │  ├─ ai\                  # nim client (reqwest + SSE), provider trait
│  │  │  ├─ notifications\       # reminder engine, reconciliation, scheduling loop
│  │  │  ├─ discord\             # webhook client (optional)
│  │  │  └─ security\            # keyring wrapper, redaction
│  │  ├─ scheduler\              # score.rs, constraints.rs, allocation.rs,
│  │  │                          # queue.rs, replan.rs, validate.rs
│  │  ├─ state.rs                # app state (pool, providers, handles)
│  │  └─ main.rs / lib.rs        # setup: plugins, tray, shortcuts, timers
│  ├─ migrations\                # versioned *.sql
│  └─ capabilities\              # minimal permission sets per plugin
│
├─ tests\                        # integration + E2E (tauri-driver) later
└─ package.json / Cargo.toml
```

### 4.1 Dependency direction

`commands → application → domain ← infrastructure` and
`application → infrastructure (interfaces) ← repositories`. Domain has **zero**
dependencies on Tauri or SQLite — it is a plain library crate so scheduler and
policy tests run headless (no window, no webview).

---

## 5. Domain-Driven Design

### 5.1 Bounded contexts

| Context | Owning module | Responsibility |
|---|---|---|
| **Strategic Planning** | `domain::goals` | Quarterly goals, milestones, success criteria, quarterly reviews |
| **Project Management** | `domain::projects` | Projects, tasks, dependencies, progress, workload |
| **Capture** | `domain::inbox` | Brain dump, classification states, classification proposals |
| **Scheduling** | `scheduler/` | Time blocks, flexible queue, capacity, constraints, feasibility |
| **Execution** | `application::SessionService` | Focus sessions, time tracking, task transitions |
| **Autonomy** | `domain::autonomy` | Action classification, policy rules, approvals, audit |
| **Engagement** | `infrastructure::notifications`, `infrastructure::discord` | Reminders, digests, webhooks |
| **AI Contracts** | `src/ai` (TS) | Prompts, context assembly, Zod validation of LLM output |

The **Scheduling** and **Autonomy** contexts are the strategic core; they depend
only on the domain model, never on AI or UI.

### 5.2 Aggregates and entities

An **aggregate** is the consistency boundary for a transaction.

| Aggregate | Roots & entities | Transaction scope |
|---|---|---|
| `InboxItem` + `ClassificationProposal[]` | inbox context | Capture: insert raw item; classify: append proposal; approve: commit snapshot + optional task/project materialization |
| `Project` + `Task[]` + `TaskDependency[]` | project context | Task CRUD, progress/workload recompute on task completion |
| `Goal` + `Milestone[]` | strategic context | Existence/status changes; only via `REQUIRE_CONFIRMATION` |
| `DailyPlan` + `ScheduledBlock[]` + `FlexibleQueueItem[]` | scheduling context | One transaction per plan commit/replan (plan + blocks + queue + audit event) |
| `AutonomyDecision` + `ApprovalRequest` | autonomy context | Every autonomy write is one append-only insert; approvals resolve in a second transaction |
| `TaskSession` | execution context | Start/stop/annotate; aggregates actual time into the task |
| `Review` (Daily/Quarterly) | reviews | Captures AI findings; read-mostly, write-once |

### 5.3 Value objects

`Priority` (0..100, typed), `Score` (0..1), `Duration` (minutes, >0),
`UtcMillis` newtype, `LocalDate`, `TimeWindow(startMs,endMs)`, `EnergyLevel`,
`BlockType`, `AutonomyLevel`, `ActionType`, `Status` enums per entity,
`AutonomyPolicySnapshot` (rules matching result), `FeasibilityReport`,
`CapacityLedger` (used/free/reserve), `ContextSelection` (retrieved entities +
token count). All enforced with Rust newtypes / `serde` + TS `zod` mirror.

### 5.4 Core domain invariants

1. A task is scheduled into **exactly one** artifact per day: either a
   `scheduled_blocks` row or a `flexible_queue_items` row. Enforcement: planner
   writes both tables in one transaction; a `CASE`-style assertion in
   `validate_schedule()`.
2. `scheduled_end > scheduled_start`; blocks of type `BREAK`/`BUFFER` never
   reference a task.
3. A block `status=ACTIVE` is never mutated by replanning (freeze rule).
4. A task with unresolved dependencies (`task_dependencies` → predecessor
   `status != COMPLETED`) is **not schedulable** today.
5. Goals/milestones/dates that are `FIXED_COMMITMENT` or strategic are only
   touched through autonomy level `REQUIRE_CONFIRMATION`.
6. Queue `score` and `position` are derived; a manual reorder writes `score`
   via a deterministic recompute, never by trusting raw LLM priorities.
7. `actual_minutes` accumulates via `task_sessions`; completion stamps
   `completed_at`; reopen requires a typed transition, not a silent flip.
8. No schedule may exceed `max_planned_minutes` or violate a hard constraint —
   otherwise `commit_plan` returns `PlanInfeasible { overflow: Vec<OverflowItem> }`.

### 5.5 Status machines (task)

`INBOX → TODO → SCHEDULED` (planner) → `IN_PROGRESS` (session start) ↔
`PAUSED` → `COMPLETED | CANCELLED`. Transitions are methods on the task entity;
invalid transitions are compile-time-illegal calls + runtime errors. Same
rigor applies to blocks (`PLANNED → ACTIVE → COMPLETED | SKIPPED`),
`inbox_items` (`RAW → CLASSIFYING → CLASSIFIED/PROPOSED → CONVERTED/DISMISSED`).

---

## 6. Hybrid Scheduling Engine

### 6.1 Inputs

Goal pipeline input = active goals + milestones; project pipeline = active
projects (priority, workload, deadlines); task pipeline = schedulable tasks
(`status IN (TODO, SCHEDULED)`, dependencies resolved, not archived); hard
constraints = fixed commitments, unavailable periods, mandatory deadlines,
user-defined commitments; soft constraints = working hours, deep-work windows,
energy patterns, buffer/break cadence, workload guardrails.

### 6.2 Partitioning

```
is_deep_work(task) = requires_deep_work
                  OR estimated_minutes >= MIN_DEEP_BLOCK(60)
                  OR importance >= DEEP_WORK_IMPORTANCE_THRESHOLD
```

- **Block-track:** deep-work-eligible tasks with estimates ≥ `MIN_FOCUS_BLOCK`
  (default 25 min) map to `scheduled_blocks`.
- **Queue-track:** everything else maps to `flexible_queue_items`.
- A task is never split implicitly; only `is_splittable=1` tasks may be split,
  and only into chunks ≥ `MIN_SPLIT_CHUNK` (default 25 min), preserving the
  original task id via multiple blocks in the same plan.

### 6.3 Constraint model

```
HARD (any violation ⇒ rejection, never silent):
  fixed commitments & appointments (MEETING / FIXED_COMMITMENT blocks, user-owned)
  UNAVAILABLE periods
  mandatory deadlines (flag on task)
  task_dependencies (predecessors must be COMPLETED before scheduling)
  workload caps: max_planned_minutes, max_deep_work_blocks,
                 max_consecutive_focus_minutes, min_break_minutes
  break/buffer placement rule (buffer ≥ min between adjacent focus blocks)

SOFT (score penalties, never absolute):
  preferred working hours, deep work windows (reward placement inside them)
  energy curve: HIGH-energy tasks early, LOW-energy tasks in recovery windows
  context-switch cost: cluster same-project/context blocks, penalize switches
  postponement_count penalizes already-delayed tasks positively (§6.4)
```

### 6.4 Priority scoring model

Configurable weights stored in `app_settings['scoring.weights']` (defaults
below). Score ∈ [0,1]; the LLM may **adjust inputs** (importance, urgency) but
never inject a raw score.

```
score(t) = Σ wᵢ · fᵢ(t),  Σ wᵢ = 1
f(goal_alignment)   = max goal priority of t's projects, normalized [0..1]
f(deadline_urgency) = clamp(1 - (deadline-now)/TIGHT_WINDOW, 0, 1)   # TIGHT=72h
f(importance)       = task.importance / 100
f(project_priority) = max project priority / 100
f(dependency_ready) = 1 if all predecessors completed else 0 (hard gate anyway)
f(estimated_effort) = 1 - (est - MIN)/(MAX - MIN)      # smaller wins slightly
f(energy_fit)       = energy match vs current window model (computed at placement)
f(context_switch)   = penalties applied as a reorder score, not a primary factor
f(postponements)    = min(postponement_count * DELAY_BONUS, 0.15)   # anti-marching-army
f(execution_history)= task-type success rate & typical duration vs estimate

default weights: goal .25 · deadline .20 · importance .15 · project .10
                 effort .05 · dependency .05 · energy .05 · history .10 · delay .05
```

### 6.5 Slot generation

1. Build a day timeline from `schedule.workingHours` (configurable, e.g.
   09:00–13:00 / 14:00–18:00).
2. Burn fixed constraints into the timeline (commitments, unavailable).
3. Emit `DeepWorkWindow`s where the user prefers deep work; keep ≥
   `MIN_DEEP_BLOCK` contiguous (never fragment below the floor).
4. Interleave `BREAK` blocks at the configured cadence and
   `BUFFER` blocks before/after fixed commitments (default 10 min).
5. Apply `max_planned_minutes` and a **workload reserve** (default 15%
   of available, min 30 min) — unused capacity stays empty by design.

### 6.6 Time-block allocation

Deterministic greedy with EDD-aware ordering (no optimization-solver dependency
in v1 — validated explicitly by the spec).

```
1. candidates  = deep_work_tasks sorted by score DESC
2. windows     = available DeepWorkWindows sorted by start
3. for t in candidates:
     if t has deadline: preferred = latest window ending >= deadline? none: strict EDD push
     w = best window for t by: (a) fits t.est, (b) energy fit, (c) context
         continuity (same project/tag as window) — scored, soft
     if none fits → t → flexible-track overflow (see 6.8)
     else: place block(t, w), refresh w remainder; if remainder < MIN_FOCUS_BLOCK
           → fold remainder into BUFFER
4. fragment blocks only if is_splittable and est > remainder+MIN_SPLIT_CHUNK
5. enforce max_consecutive_focus: split long runs with BREAK blocks
6. final: validate_schedule(file); on failure rollback and switch offending
   tasks to queue/overflow
```

**Hard-deadline heuristic:** before the greedy loop, tasks with mandatory
deadlines are tried first in EDD order using a longest-window-fit, because a
greedy by score can starve a tight deadline. This is the only deviation from
pure score order and it is deterministic.

### 6.7 Flexible queue selection

```
select_best_for_gap(gap_minutes, now_context):
   among QUEUED items where est <= gap_minutes
      and context/energy compatible
      and not the active task
      and est <= gap - trailing_buffer (default 5)
   pick argmax score, tie-break: earliest deadline, then lowest context cost
   mark SELECTED → UI suggests "Start [task]" in the Today panel
```

The queue answers "what fits the next 25 minutes?" — it never fabricates a time
slot for something that doesn't fit. Tasks larger than the next gap stay QUEUED.
Splitting is applied **only** when `is_splittable` and a fair chunk fits.

### 6.8 Feasibility & overflow

After allocation, remaining queued work is compared against remaining flexible
capacity. If it exceeds it, the plan persists **with an explicit overflow
report** (never an overloaded day):

```
PlanInfeasible {
  overflow: [ { task_id, est, reason: DEADLINE_UNMET | CAPACITY | DEPENDENCY },
              ... ],
  suggested_deferrals: [ task_id ],          # lowest-value first, per §6.4
  displaced_soft_blocks: []                  # only when policy approves
}
```

`daily_plans.feasibility` stores this JSON. The Today screen renders it as a
"will not fit" list with one-tap postpone actions (through the autonomy engine).

### 6.9 Replanning protocol

1. Snapshot the current plan (`schedule_events` row with `undo_payload`).
2. Freeze: `now..now+ACTIVE_WINDOW(30m)` — nothing before that horizon moves
   (protects the active task).
3. Re-run §6.5→6.8 from the horizon forward.
4. Compute diff (blocks added/moved/cancelled, queue reorders).
5. If the diff touches strategic/fixed items → `PROPOSE`/`REQUIRE_CONFIRMATION`.
   Otherwise if the displacement is ≤ configured threshold → apply (AUTO) and
   audit; larger → propose.
6. Bump `daily_plans.version`, set `planning_mode=ADAPTIVE`, increment
   `adaptive_mutations`, fire `SCHEDULE_CHANGE` notification (and optional
   Discord digest).

### 6.10 Adaptive scenarios (event → policy → action)

| Scenario | Detection | Base level | Handling |
|---|---|---|---|
| **A Early completion** | session end < scheduled end | AUTO | Reach capacity, select best next flexible task, update queue & estimates; audit |
| **B Delayed execution** | session still ACTIVE past block end | AUTO (keep active) | Preserve active block, extend it, recompute only future blocks; notify if a deep-work block is displaced → PROPOSE |
| **C New urgent task** | inbox item classified + urgency ≥ threshold | PROPOSE | Analyze displacement; if insert fits free or reserve capacity → AUTO insert flexible; if it displaces a deep-work block → PROPOSE |
| **D Skipped task** | skip action | - | Prompt postpone/reschedule/discard; re-evaluate score; postpone via queue → AUTO, block move → PROPOSE, discard → REQUIRE_CONFIRMATION |
| **E Unexpected unavailability** | `UNAVAILABLE` block created spanning `now` | PROPOSE (if displaces later blocks) | Preserve completed, protect fixed commitments, recompute, reduce workload, present revised plan |
| **F No realistic schedule** | overflow non-empty | - | Never overload; identify lowest-value items; recommend postponements; surface "will not fit" list |

---

## 7. Adaptive Autonomy Engine

### 7.1 Action taxonomy and risk classification

The engine is a **deterministic, configurable policy engine**. The LLM proposes
an `Action`; `AutonomyPolicy::classify(action, context)` returns a level. The
LLM can never override it — the policy function is pure Rust, unit-tested, and
its rule table is user-editable in Settings.

| Action type (examples) | Default level | Policy rule id |
|---|---|---|
| `mark_task_completed` (explicit user action) | AUTO_EXECUTE | R01 |
| `reorder_flexible_queue` | AUTO_EXECUTE | R02 |
| `insert_flexible_task` (fits free capacity) | AUTO_EXECUTE | R03 |
| `adjust_estimate` (from approved execution history) | AUTO_EXECUTE | R04 |
| `recalc_remaining_capacity` | AUTO_EXECUTE | R05 |
| `move_deep_work_block` | PROPOSE | R10 |
| `reschedule_project_task` (non-trivial) | PROPOSE | R11 |
| `insert_task_displacing_priority` | PROPOSE | R12 |
| `substantial_day_replan` | PROPOSE | R13 |
| `adopt_inbox_classification` (creates tasks/projects) | PROPOSE | R14 |
| `change_goal` / `change_milestone` | REQUIRE_CONFIRMATION | R20 |
| `delete_task` / `delete_project` / `abandon_project` | REQUIRE_CONFIRMATION | R21 |
| `change_mandatory_deadline` | REQUIRE_CONFIRMATION | R22 |
| `move_fixed_commitment` | REQUIRE_CONFIRMATION | R23 |
| `change_quarterly_objective` | REQUIRE_CONFIRMATION | R24 |

### 7.2 Policy rule evaluation (pure function)

```
classify(action, ctx) -> Level:
  for rule in rules(action.type):                  # ordered, first match wins
      if rule.matches(ctx):                        # boolean predicates only
          return rule.level
  return DEFAULT_LEVEL(action.type)

rule = { id, action_type, level, predicates: [ (field, op, value) ], reason }
predicates operate on: action.target (entity) + entity.status,
schedule_delta_minutes, touches_strategic(bool), touches_fixed(bool),
displaces_active(bool), within_reserve(bool)
```

Rules are stored as JSON in `app_settings['autonomy.rules']` merged over the
compiled-in defaults, so the user can loosen/tighten levels without code
changes. Hard-coded rules (R20–R24) **can only be tightened**, never loosened
below REQUIRE_CONFIRMATION.

### 7.3 Decision execution flow

```
1. trigger detected (user action / scheduler event / inbox classify)
2. AI layer (if applicable) produces a Zod-validated ProposedOperation
3. AutonomyPolicy.classify → level
4. match level:
     AUTO_EXECUTE         → apply via ApplicationService (transaction),
                            append autonomy_decisions(EXECUTED),
                            append schedule_events(undo_payload)
     PROPOSE              → persist autonomy_decisions(PROPOSED) +
                            approval_requests(PENDING), push UI event
     REQUIRE_CONFIRMATION → same as PROPOSE, flagged high-impact; modal
5. user resolves → service re-validates payload (may have gone stale) →
   apply or reject → decision status flips; undo event recorded
```

### 7.4 Undo

Every applied operation stores a JSON `undo_payload` in `schedule_events`
(e.g. `{op:"restore_block", block_id, previous:{...}}`). `undo(decision_id)`
re-executes the inverse in a transaction and flips `schedule_events.reverted`.
Entities are soft-deleted; nothing is ever hard-deleted silently.

### 7.5 Audit trail

`autonomy_decisions` rows: action_id, action_type, timestamp, trigger, level,
matched policy snapshot, status, reason, before/proposed/result payloads,
related task ids, approval link, undo link — queryable in Analytics and
rendered as an audit timeline.

---

## 8. NVIDIA NIM Integration

### 8.1 Verified API contract (researched from official docs, Nov 2026)

- **Hosted base URL:** `https://integrate.api.nvidia.com/v1` — OpenAI-compatible
  (verified via `build.nvidia.com/llms.txt` and the official API Reference).
- **Auth:** `Authorization: Bearer <NVIDIA_API_KEY>` (generated at
  `build.nvidia.com/settings`). Stored strictly in the OS keyring.
- **Endpoint:** `POST /v1/chat/completions` (multi-turn; streaming + tool
  calling). Also `/v1/completions`, `/v1/responses`, `/v1/models`, `/tokenize`.
- **Structured output:** `response_format: {"type":"json_schema","json_schema":{…}}`
  constrains generation at decode time — the preferred mode. Plain
  `"json_object"` mode is supported on OpenAI-compatible stacks but is weaker.
- **Streaming:** SSE `data:` deltas, terminated by `data: [DONE]`; hosted API
  defaults `stream=true`.
- **Sampling:** `temperature`, `top_p` (hosted default 0.95), `seed` (best-effort
  determinism), `max_tokens` (hosted 1…65536, default 1024), `stop`.
- **Async semantics:** hosted API may return `202` + `requestId`; client polls.
- **Model naming:** `{org}/{model}`, e.g.
  `nvidia/llama-3.3-nemotron-super-49b-v1`, `deepseek-ai/deepseek-v4-pro`,
  `openai/gpt-oss-120b`. Models differ (reasoning vs instruct, context length,
  json_schema support) — a **capability matrix** accompanies each configured
  model in Settings.
- **Self-hosted NIM:** same OpenAI-compatible surface + management endpoints
  (`/v1/health/ready`, `/v1/models`, `/tokenize`, `/docs`). The serving engine
  is vLLM or SGLang, which can restrict tool/guided-decoding features; the
  client probes `/v1/models` and enables features accordingly.

### 8.2 Provider abstraction

```rust
trait LlmProvider: Send + Sync {
    async fn chat(&self, req: ChatRequest) -> Result<ChatResponse, LlmError>;
    async fn stream(&self, req: ChatRequest) -> Result<Stream<Item = Chunk>, LlmError>;
    fn capabilities(&self) -> ProviderCapabilities;  // json_schema? tools? max_ctx?
}

struct NimProvider  { config: NimConfig, client: reqwest::Client }
struct MockProvider { canned: Vec<ChatResponse> }    // tests + offline demo

ChatRequest { model, messages, temperature?, max_tokens?, response_format?,
              stream, seed?, tools? }
```

`NimConfig` (stored in `app_settings`, secrets excluded): base_url, model,
api_key_ref (keyring entry name), timeout_ms, retries (default 3, exponential
backoff + jitter on 429/5xx/timeout), token budgets per operation.

### 8.3 Per-operation service catalog (TS orchestrators, Zod schemas)

| Service | Purpose | Output schema |
|---|---|---|
| `BrainDumpAnalyzer` | classify raw text: kind, project guess, goal link, duration, priority, deadline, actionability, dependencies, next step | `BrainDumpAnalysis` |
| `TaskClassifier` | classify a task into the domain vocabulary | `TaskClassification` |
| `ProjectAnalyzer` | cluster inbox entries into projects; detect relationships | `ProjectClustering` |
| `GoalPlanner` | decompose a quarterly goal → milestones, themes, risks | `GoalPlan` |
| `DailyPlanner` | main objective, top-3 priorities, workload hints | `DailyPlanProposal` |
| `ScheduleOptimizer` | alternative placements + rationale for hypothetical moves | `ScheduleSuggestion` |
| `ScheduleChangeExplainer` | "why did this move?" — rendered inline in the UI | `ChangeExplanation` |
| `DailyReviewAnalyzer` | accomplishments, blockers, planning accuracy | `DailyReviewAnalysis` |
| `QuarterlyReviewAnalyzer` | outcomes, lessons, next-quarter recommendations | `QuarterlyReviewAnalysis` |

All outputs pass `Zod.parse`, then `verify_entity_refs()` (every referenced id
must exist and fit its role — resolved against the DB), then become
`ProposedOperation[]` for the autonomy + service layer.

### 8.4 Pipeline (trust boundaries)

```
context assembly (small, token-budgeted; FTS5 top-k retrieval)
   → NimProvider.chat / stream (Rust: retries, timeout, SSE, 202 polling)
   → raw text / JSON
   → Zod parse (TS)                 — structural validation
   → verify_entity_refs (TS → Rust) — referential validation
   → map to ProposedOperation[]     — semantic narrowing
   → AutonomyPolicy.classify (Rust) — risk classification
   → ApplicationService.execute (Rust, transaction)   ⬅ the ONLY writer
```

`ai_planning_history` records tokens, status code, raw and validated outputs,
and a **content-addressable hash** of the context payload for replay without
persisting the whole database.

### 8.5 Context selection & token budgeting

- Never send the whole DB. `ContextSelector` fetches only: active goals +
  milestones (titles, status), active project summaries, today's plan blocks,
  the inbox batch under analysis, and relevant history via SQLite **FTS5**
  (top-k by relevance). That payload is then truncated deterministically
  (drop least-relevant fields first) against a per-operation token budget.
- Per-operation budgets (settings): classify `700` / plan `2000` ·
  review `2500` completion tokens; prompt payload capped by computed
  ~char-to-token ratio before dispatch.
- `seed` fixed per operation type during development to reduce flakiness;
  disabled in production unless the user opts into reproducibility.

### 8.6 Failure & degradation

- **Provider down / timeout:** operation marked `SKIPPED` in history;
  classification re-queued; daily planning falls back to the deterministic
  scheduler seeded with the *last approved plan* + heuristic scores. All core
  CRUD and local scheduling remain fully functional.
- **Zod validation failure:** one bounded retry with a repair prompt; second
  failure = `VALIDATION_ERROR`, **no state mutation**, user sees raw output
  with a "Regenerate" affordance.
- **202 async:** poll with backoff up to the operation's deadline; on timeout,
  treat as provider failure.
- The app **never blocks** business logic on the LLM; every AI interaction is
  asynchronous with a visible status chip.

### 8.7 Settings → AI

Settings exposes: provider (NIM hosted / self-hosted / mock), base URL, model
name (+ capability matrix readout), API key (Saved to OS keyring), timeout,
retries, temperature, per-operation token budgets, and a global AI-enable switch.

---

## 9. Tauri / React Application Shell

### 9.1 Process & capabilities

- Single main window; **system tray** (built-in `tray-icon`) keeps the process
  alive when the window closes (hide-to-tray, with platform notes in §9.4).
- Capabilities (`src-tauri/capabilities/*.json`): main window gets only
  `core:default`, `notification:default`, `global-shortcut:allow-register…`,
  and the app's own command set; the tray runs `core:tray` + menu items.
  No `tauri-plugin-shell` for arbitrary command execution, no fs writes outside
  the app data dir, strict CSP.
- `tauri-plugin-single-instance`: second launch focuses the existing window
  and opens the focus-mode or quick-capture view via a startup-payload channel.
- Plugins: notification, global-shortcut, single-instance, autostart,
  window-state, opener, log, keyring (community, OS backends).

### 9.2 Typed IPC contract

Every `#[command]` returns `Result<T, AppError>` where `T` is a serde DTO.
`tauri-specta` generates `src/shared/contracts.ts` from the DTOs at build time,
so drift between Rust and TS is a compile error, not a runtime surprise.
Commands are grouped: `inbox_*`, `task_*`, `project_*`, `goal_*`, `plan_*`,
`scheduler_*`, `session_*`, `review_*`, `autonomy_*`, `settings_*`, `ai_*`,
`notifications_*`, `discord_*`, `analytics_*`. Async long-runs (planning,
AI calls) emit progress via the Tauri **event** system.

### 9.3 Notification & reminder engine (Rust, tokio)

1. On startup and after each plan commit, the engine materializes future
   notifications as rows (`REMINDER_30M` at `start-30m`, `REMINDER_START` at
   `start`, `OVERDUE` at `end+grace`) with dedupe against existing rows
   (`(kind, block_id, scheduled_for)` unique). **Dedupe is persistence-backed**:
   restarts cannot double-fire.
2. A heartbeat task (60 s tick) scans `status=PENDING AND scheduled_for<=now`,
   checks preferences + focus-session suppression, emits via
   `tauri-plugin-notification`, marks `SENT` (or `SUPPRESSED`).
3. **Missed-notification reconciliation:** on every app start, rows that fell
   between `scheduler_state.watermark` and `now` are flushed as `MISSED` and a
   single digest notification is shown. Watermark moves only after flush.
4. Focus suppression: no notification during an ACTIVE focus session except
   the session's own end/pomodoro events (configurable).
5. **Platform limitation (documented in-app):** reminders fire only while the
   Focus OS process is running (window open or tray-resident). If the app is
   fully quit, missed reminders are reconciled on next launch. A
   companion helper-service is a deliberate non-goal for v1.

### 9.4 System tray, global shortcuts, lifecycle

- Tray: Today / Quick Capture / Focus Mode / Pause reminders / Quit.
  Closing the main window hides to tray (Windows/macOS); Real quit via menu.
- Global shortcut registry (defaults, configurable): `Ctrl+Shift+Space`
  quick capture, `Ctrl+Shift+F` focus mode, `Ctrl+Shift+T` open Today.
  Registration failures (shortcut taken → conflict) are surfaced non-fatally.
- Autostart (optional setting) via `tauri-plugin-autostart`.
- Window-state restores geometry; single-instance prevents data mutation races.

### 9.5 Security & secrets

- API key + Discord webhook URL live in the **OS keyring**
  (`tauri-plugin-keyring`); `app_settings` stores only opaque non-secret
  references (`keyring://nvidia.api_key`). Fallback: stronghold vault if the
  platform keyring is unavailable, flagged in Settings.
- `NimConfig` redacts secrets from all logs (structured `log` + `tracing`).
- CSP: `default-src 'self'; connect-src ipc: http://ipc.localhost` + the *user-
  configured* NIM origin; `style-src 'self' 'unsafe-inline'` (Tailwind);
  no remote script eval.
- MVP threat scope: local storage, OS keyring, outbound HTTPS to a
  user-configured host. No telemetry. (Review table in §13.)

### 9.6 Discord (optional, webhook)

`DiscordService` (Rust): daily digest (date, main objective, top-3 priorities,
block list, flexible tasks, deadlines, one-line AI summary) and optional
`SCHEDULE_CHANGE` posts. Webhook URL fetched from keyring at send time;
timeouts, retries (2), errors logged and swallowed. Fully usable without
Discord — the flag merely gates the digest composition.

---

## 10. User Interface

### 10.1 Design system

Dark-first, minimal, premium-productivity aesthetic. Tailwind tokens as CSS
variables: `--bg`, `--surface`, `--surface-2`, `--text`, `--text-muted`,
`--accent`, `--success`, `--warning`, `--danger`, `--border`. Typography:
system stack (Inter UI defaults) with tight tracking; 4-step type scale.
Motion: 120–160 ms ease-out transitions only (timers, plan changes, overlays);
no parallax, no celebratory animations. Every screen answers "what to do
next" via a persistent **Next Action Rail**.

### 10.2 Information architecture

```
┌───────────────┬────────────────────────────────────────────────────┐
│  Focus OS     │  <screen content>            [capture +] [timer]   │
│  ─────────    │                                                    │
│  ▶ Today      │                                                    │
│    Inbox  3   │                                                    │
│    Projects   │                                                    │
│    Goals      │                                                    │
│    Analytics  │                                                    │
│    Settings   │                                                    │
│  ─────────    │                                                    │
│  ⌘⇧Space      │                                                    │
│  ⌘⇧F Focus    │                                                    │
│  [tray]       │                                                    │
└───────────────┴────────────────────────────────────────────────────┘
```

Routes: `/today`, `/inbox`, `/projects`, `/projects/:id`, `/goals`,
`/goals/:id`, `/analytics`, `/settings` (`/settings/ai`, `/settings/schedule`,
`/settings/notifications`, `/settings/autonomy`, `/settings/discord`,
`/settings/appearance`). **Focus Mode** is a separate borderless window
(opened via tray/shortcut); quick capture is an app-level overlay reachable
from anywhere + a global shortcut.

### 10.3 Wireframe — Today

```
┌─ Today · Wed, 11 Nov ──────────────────────────────────────────────┐
│ MAIN OBJECTIVE — "Ship Focus OS v0.1 MVP"                         │
│                                                                    │
│ TOP 3  [1] Complete hybrid planner   [2] NIM classify pipeline     │
│        [3] Inbox triage  ·  Progress ▓▓▓░░ 42%                    │
│                                                                    │
│ ╭─ TIMELINE ────────────────╮  ╭─ FLEXIBLE QUEUE ───────────────╮  │
│ │ 09:00 ▸ DEEP WORK          │  │ ◐ 1 Review PR #214  25m       │  │
│ │      Planner architecture  │  │   2 Inbox triage    20m       │  │
│ │ 09:40 ▿ BREAK              │  │   3 Update docs     15m       │  │
│ │ 10:00 ▸ MEETING (fixed)    │  │ ⤷ next gap 24m → fits #1      │  │
│ │ 11:15 ▸ DEEP WORK          │  ╰───────────────────────────────╯  │
│ │      NIM integration ◆adpt │  ╭─ WILL NOT FIT ────────────────╮  │
│ │ 12:30  buffer/reserve 1h40 │  │ 4 Ship release notes  45m     │  │
│ ╰════════════════════════════╯  ╰───────────────────────────────╯  │
│ NOW [Start · Review PR #214]   · Next 14:00 Deep Work — 30m pre   │
│ Remaining 3h20m · Reserve 40m · Plan adapted 1× (⟳ explain)       │
└───────────────────────────────────────────────────────────────────┘
```

Answers *what matters today* (objective + top-3), *what now* (active block +
timer), *what next* (upcoming gap + queue fit + "Start [task]" CTA), *what
won't fit* (overflow panel with one-tap postpone). "Plan adapted" badge +
"⟳ explain" opens `ScheduleChangeExplainer` (AI + deterministic diff).

### 10.4 Wireframe — Inbox

```
┌─ Inbox ──────────────────────────────────────[⌘⇧Space]────────────┐
│ [ input — type anything, press Enter to capture instantly ]       │
│                                                                    │
│ ◦ 12:31 "ship weekly build even if release notes slip"   ⟳CLASS    │
│   suggestion: TASK · project: Focus OS · 25m · Tue      [apply]   │
│ ◦ 12:18 "idea: energy watermark in queue scoring"        [apply]   │
│   suggestion: IDEA · no project · needs decomposition             │
│ ◦ 11:52 raw, unprocessed                                       ⟳ai │
└───────────────────────────────────────────────────────────────────┘
```

Each row: captured raw text (never altered), source tag, AI suggestion chip
with confidence + consequence preview (creates task? links project? duration?),
per-row [Apply] / [Dismiss]. Raw items save instantly — no AI in the write
path. Classifications are asynchronous, inspectable, and reversible.

### 10.5 Wireframe — Focus Mode (separate borderless window)

```
┌─ Focus — Deep Work ──────────────────────────────────────────────┐
│  Planner architecture · Focus OS · Goal Q4 · est 2h              │
│                   ⏱ 01:24:07 / 02:00:00 (actual)                 │
│  [Pause]  [Complete]  [Skip →]    🔔 suppress-on                 │
│  ─────────────────────────────────────────────  quick capture    │
└──────────────────────────────────────────────────────────────────┘
```

Quick-capture overlay floats above: input + save → `inbox_items`, zero effect
on the session. Pomodoro optional (settings), never forced.

### 10.6 Remaining screens (spec-level)

- **Projects:** status-prioritized list (progress bar, workload) → detail
  (description, linked goals, tasks grouped by status, dependency drawer,
  notes, est vs actual workload, AI clustering affordance for inbox entries).
- **Goals:** quarterly objective cards → drill-in (milestones, associated
  projects, target vs actual progress, "Decompose with AI" via `GoalPlanner`
  → approval flow, quarterly review entry point). Never auto-mutated.
- **Analytics:** time distribution by project/type; execution history
  (sessions); planning accuracy (est vs actual); goal progress trend;
  recurring blockers (from daily reviews); autonomy audit timeline
  (filterable by level/status/actor).
- **Settings:** AI · Schedule · Notifications · Autonomy · Discord · Appearance
  · Data (migrations, export/backup, restore). Autonomy screen renders the
  policy table with level dropdowns (escalate-only for R20+).

---

## 11. Testing & Quality

### 11.1 Rust unit tests (`cargo test`, headless)

| Target | Cases |
|---|---|
| Priority scoring | weight config, bounds, deadline urgency ramp, dependency gate, effort/energy/history factors |
| Hybrid scheduling | partition rules, slot generation, allocation order, EDD dead-line handling, no fragment-below-floor, active-block freeze |
| Hard constraints | overlap rejection, workload caps, break cadence, buffer rule, dependency unmet, unavailable-period violation |
| Flexible queue | gap-fit selection, trailing buffer, argmax + tie-breaks, no overfit, splittable-only splitting |
| Recalculation | early-completion, delay, scenario A/B/E/F (property-based where cheap) |
| Autonomy policy | rule table default levels, predicate matching, escalate-only R20+, order of rules |
| Task transitions | full state machine, illegal transitions panic/error |
| Goal progress | milestone→goal progress math, derived progress snapshots |
| AI output validation (mock) | Zod rejects malformed JSON schema violations, entity-ref resolver rejects phantom IDs, retry+repair path |
| Undo | each operation's inverse restores exact prior state, reverted flag flips |

### 11.2 Rust/SQLite integration tests

In-memory or temp-file DBs (WAL): migrations apply cleanly v0→latest and are
idempotent by version; FK/CHECK violations surface typed errors; transaction
atomicity (plan commit that fails half-way leaves no partial rows);
approval workflow commit/reject round-trip; notification dedupe across
"restart" (new connection, same file); scheduler_state watermark recovery.

### 11.3 TS tests (`vitest`)

Zod schemas (valid/invalid fixtures), context-selector payload budgets,
orchestrator mapping of validated output → `ProposedOperation[]`, store
reducers, React component smoke tests.

### 11.4 NIM mocking

`MockProvider` (deterministic canned responses) everywhere; `wiremock` tests
for the real `NimProvider`: header injection, retry/backoff on 429/5xx,
timeout, SSE parsing (`data:`/`[DONE]`), 202 polling, JSON-schema passthrough.

### 11.5 E2E (later phases)

`tauri-driver`/WebDriver smoke suite for: capture→classify→approve→plan→focus→
review happy path; scenario C (urgent task) and F (overload) through the UI;
restart stability.

### 11.6 Gates (every phase must pass)

`cargo fmt --check` · `cargo clippy -D warnings` · `cargo test` ·
`tsc --noEmit` · `vitest run` · manual smoke checklist per phase (in
`docs/phase-notes/`). No phase ships with `TODO()` or placeholder logic.

---

## 12. Phased Roadmap

Each phase: **Goal → Deliverables → Exit criteria**. Tests + build must pass at
the end of every phase; each phase leaves the app runnable. Work is
incremental by design; no phase starts the next until its gates close.

### Phase 1 — Foundation
- Tauri 2 scaffold (Vite+React+TS strict), Tailwind, dark shell, sidebar nav;
  SQLite pool + migration runner (`v0001` = this schema, DDL from
  `docs/SQLITE_SCHEMA.sql`); settings service + app_settings store; DB test
  harness; pluggable logger; single-instance + window-state; container layout.
- **Exit:** app launches, migrations apply cleanly, `cargo test` + `vitest`
  green, empty Account-free screens render.

### Phase 2 — Core domain
- Domain entities + status machines (task/project/goal/milestone/inbox);
  repositories; InboxService (instant capture, async classify states);
  ProjectService; GoalService; task dependencies; soft-delete everywhere;
  typed commands for every aggregate.
- **Exit:** full CRUD round-trips through IPC; integration tests; capture of a
  raw item is < transaction-time and requires no AI.

### Phase 3 — Scheduling engine (deterministic, pre-LLM)
- Score model (config weights); constraint model; slot generation;
  block allocation (greedy + EDD); flexible queue + gap fitting; feasibility
  reports; daily_plan commit with versioning + schedule_event audit;
  replan protocol + scenario engines A/B/E/F; plan diff/validation tests.
- **Exit:** unit + integration tests for all §11.1 scheduling cases; Today
  renders a hybrid plan computed entirely locally.

### Phase 4 — AI intelligence
- `NimProvider`/`MockProvider` + ChatRequest + SSE + retry/timeout + wiremock
  tests; ContextSelector (FTS5) + token budgets; Zod schemas + entity-ref
  verification; orchestrators BrainDumpAnalyzer · TaskClassifier ·
  ProjectAnalyzer · GoalPlanner · DailyPlanner · ScheduleOptimizer ·
  ScheduleChangeExplainer; ai_planning_history; degradation paths.
- **Exit:** classify→propose→apply works against **mock** provider in CI;
  manual run against real NIM with diagnostics; app fully functional when
  provider is down.

### Phase 5 — Adaptive autonomy
- AutonomyPolicy + rule engine (defaults + overrides); action taxonomy;
  approval workflows UI (dialog + inbox-style queue); audit timeline;
  undo service; autonomy commands + tests; scenario C integration.
- **Exit:** audit + undo round-trip tests; policy `<->` LLM override attempt
  provably impossible (integration test asserts rejected).

### Phase 6 — Execution
- Today dashboard (timeline, queue, now/next, overflow, adapt badge);
  Focus mode window + timer + sessions; quick-capture overlay (global);
  task transitions end-to-end; daily review capture (manual draft first,
  AI fields in Phase 8).
- **Exit:** full capture → plan → focus → complete → review loop closes
  locally; time tracking aggregates into tasks/projects/goals.

### Phase 7 — Integrations
- Notification engine (materialize/dedupe/reconcile/focus suppression);
  30-min + start + overdue + daily reminders; system tray menu;
  global shortcuts; Discord digest (optional); keyring-backed secrets.
- **Exit:** restart-missed-notification test; tray/shortcut smoke on
  Windows; Discord flag toggling without crash.

### Phase 8 — Intelligence & refinement
- DailyReviewAnalyzer + QuarterlyReviewAnalyzer; duration estimation
  updates (approved history recalibration → R04); recurring-blocker
  analytics; planning-accuracy metrics; adaptive insights over time.
- **Exit:** review pipelines produce charts; est-vs-actual recalibration
  visibly improves 2-week-old plans during manual soak.

### Phase 9 — Quality & packaging
- Performance passes (query plans, index review, payload sizes); security
  review (CSP, keyring, redaction, threat table §13); cross-platform
  verification (Windows target for this session; macOS/Linux builds in CI);
  installer (NSIS/Wix), tray icon set, versioning, backup/export;
  docs/phase-notes + README; final E2E smoke.
- **Exit:** release build on supported platforms; full test matrix green;
  known-issues list complete and honest.

---

## 13. Architectural Risks & Trade-offs

| # | Risk | Mitigation | Accepted trade-off |
|---|---|---|---|
| R1 | **Two-language domain boundary** (Rust core, TS AI layer) adds upfront cost and contract drift | `tauri-specta` single-source DTOs; strict `tsc`; AI schemas are read-only Zod mirrors | Slower Phase 1–2 vs. a pure-TS app; the payoff is proven invariants at the write boundary |
| R2 | **NIM model variance**: not every model reliably honors `json_schema`; tool calling/guided decoding differ by engine (vLLM vs SGLang) and hosting | Capability matrix per configured model; capability probe on connect; Zod validation + one repair retry; degraded `json_object` path; settings-switchable model | Slightly more plumbing; never assume parity — engineered in from day one |
| R3 | **Scheduler heuristic quality**: greedy + EDD may produce suboptimal fits on pathological days | Deterministic v1 validated against scenario tests (A–F); explicit *constraint-optimization decision point* after v1 data; claims kept honest in docs | No solver dependency in v1 (spec explicitly asks to validate the simple scheduler first) |
| R4 | **Desktop background limitations**: reminders stop if the process is fully quit; no true OS service | Hide-to-tray, autostart option, single-instance, missed-notification reconciliation, in-app platform note | A companion service is explicitly out of scope for v1 |
| R5 | **Keyring availability** variance across Linux desktops | `keyring-core` with `linux-keyutils` fallback; stronghold fallback flagged in Settings | Secrets never stored in plaintext config; may require one-time unlock prompts |
| R6 | **Webview memory/GC & IPC payload size** with heavy Today data | Read-model projections sized for the view; pagination for big lists; structured diagnostic payloads | Slightly more query code vs. returning whole aggregates |
| R7 | **Security scope creep** (local-first apps are often underestimated targets) | Minimal capability grants; CSP; no shell plugin; keyring for secrets; redaction; periodic threat-model review in Phase 9 | Some convenience features (deep file access) deliberately withheld |
| R8 | **Decision-fatigue swamps**: over-engineering the autonomy matrix | Rule engine is data-driven from day one (defaults as JSON); UI exposes only safe levers | A small rules vocabulary in v1; extensible structure ready for growth |
| R9 | **AI context/token drift** as the DB grows | FTS5 top-k selection; per-op budgets; context-payload hashing for replay; history pruning policy | Slightly less holistic prompts early on; prevents ballooning cost/latency |
| R10 | **SQLite as single writer + WAL**: long transactions (plan commit) contend with UI writes | Short transactions, pool sizing, busy_timeout, read-only projection queries; commit plans batch within one tx | Small risk of `SQLITE_BUSY` on pathological patterns — covered by tests |

### 13.1 Explicitly deferred complexity

- Constraint-optimization / MILP scheduler — decision point after Phase 3 data.
- End-to-end encryption & cloud sync — non-goals for v1 (local-first posture).
- Multi-user / family accounts — single-user domain model.
- Chat-style AI interface — the product is a scheduler first; AI is contextual
  (explanations, classifications, reviews), not a chat window.

### 13.2 Glossary

- **Hybrid plan** — a `daily_plans` row + its blocks + its queue.
- **ProposedOperation** — validated, closed algebra of AI-intended mutations.
- **Level** — AUTO_EXECUTE / PROPOSE / REQUIRE_CONFIRMATION.
- **Flexible capacity** — minutes free after blocks, breaks, buffers, reserve.
- **Workload reserve** — protected unfilled capacity (default 15%, min 30 m).
- **Active window** — freeze horizon protecting the running task during replans.

---

## Appendix A — Initial settings keys

`ai.{provider,baseUrl,model,capabilities,timeoutMs,retries,temperature,
tokenBudgets.{classify,plan,review},enabled}` ·
`discord.{enabled,webhookUrlRef,dailyDigest,scheduleChangeDigest}`
`schedule.{workingHours,deepWorkWindows,maxDailyMinutes,maxDeepWorkBlocks,
minBreakMinutes,minBufferMinutes,reserveRatio,maxConsecutiveFocusMinutes,
minFocusBlockMinutes,minSplitChunkMinutes}` ·
`notifications.{reminder30m,reminderStart,overdue,dailyPlanning,endOfDay,
suppressDuringFocus,dailyPlanningTime,endOfDayTime}` ·
`autonomy.{rules,escalation,historyRetentionDays}` ·
`scoring.weights.*` · `focus.{pomodoroEnabled,roundMinutes,breakMinutes}` ·
`appearance.{theme,accent,compact}` · `store.{startMinimized,singleInstance}`.

---

*This document is the contract for implementation. Amendment requires a
design note appended to `docs/ADRs/` plus reviewer approval; the SQLite schema
floats only via explicit migrations.*