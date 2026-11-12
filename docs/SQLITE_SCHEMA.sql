-- =====================================================================
-- Focus OS — Initial SQLite Schema (migration v0001)
-- =====================================================================
-- Conventions:
--   * IDs: UUIDv7 (TEXT, time-ordered for index locality).
--   * Timestamps: epoch milliseconds (INTEGER, UTC). Calendar dates as 'YYYY-MM-DD'.
--   * Soft delete: entities carry archived / deleted_at instead of hard DELETE.
--   * Foreign keys enforced at connection level (PRAGMA foreign_keys = ON).
--   * WAL journaling + busy_timeout set by the connection manager.
--   * JSON payloads and explicit user-authored text fields are the sanctioned
--     free-form columns; everything else is normalized and strictly typed.
--   * v0002 adds the goal Sidecar as a separate observation stream.
-- =====================================================================

-- Settings: key/value JSON store (avoids migration churn for preferences).
CREATE TABLE app_settings (
    key        TEXT PRIMARY KEY,
    value      TEXT NOT NULL,          -- JSON-encoded settings blob
    updated_at INTEGER NOT NULL
);

-- =====================================================================
-- Strategic layer: goals -> milestones
-- =====================================================================
CREATE TABLE goals (
    id               TEXT PRIMARY KEY,                       -- uuid v7
    title            TEXT NOT NULL,
    description      TEXT,
    motivation       TEXT,
    expected_outcome TEXT,
    success_criteria TEXT,                                   -- JSON array of measurable criteria
    start_date       INTEGER,
    target_date      INTEGER,
    priority         INTEGER NOT NULL DEFAULT 0,             -- 0..100
    status           TEXT NOT NULL DEFAULT 'PLANNED'
        CHECK (status IN ('PLANNED','ACTIVE','COMPLETED','ABANDONED')),
    progress         REAL NOT NULL DEFAULT 0
        CHECK (progress >= 0 AND progress <= 1),
    created_at       INTEGER NOT NULL,
    updated_at       INTEGER NOT NULL,
    archived         INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE milestones (
    id           TEXT PRIMARY KEY,
    goal_id      TEXT NOT NULL REFERENCES goals(id) ON DELETE CASCADE,
    title        TEXT NOT NULL,
    description  TEXT,
    target_date  INTEGER,
    status       TEXT NOT NULL DEFAULT 'PENDING'
        CHECK (status IN ('PENDING','IN_PROGRESS','ACHIEVED','SKIPPED')),
    position     INTEGER NOT NULL DEFAULT 0,
    created_at   INTEGER NOT NULL,
    updated_at   INTEGER NOT NULL
);

-- =====================================================================
-- Project layer
-- =====================================================================
CREATE TABLE projects (
    id                  TEXT PRIMARY KEY,
    name                TEXT NOT NULL,
    description         TEXT,
    status              TEXT NOT NULL DEFAULT 'BACKLOG'
        CHECK (status IN ('BACKLOG','ACTIVE','ON_HOLD','COMPLETED','ARCHIVED')),
    priority            INTEGER NOT NULL DEFAULT 0,           -- 0..100
    start_date          INTEGER,
    deadline            INTEGER,
    estimated_workload  INTEGER,                              -- minutes
    actual_workload     INTEGER NOT NULL DEFAULT 0,           -- minutes
    progress            REAL NOT NULL DEFAULT 0
        CHECK (progress >= 0 AND progress <= 1),
    notes               TEXT,
    created_at          INTEGER NOT NULL,
    updated_at          INTEGER NOT NULL,
    archived            INTEGER NOT NULL DEFAULT 0
);

-- Many-to-many: projects <-> goals (a project may serve several objectives).
CREATE TABLE project_goals (
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    goal_id    TEXT NOT NULL REFERENCES goals(id) ON DELETE CASCADE,
    role       TEXT NOT NULL DEFAULT 'PRIMARY'
        CHECK (role IN ('PRIMARY','SECONDARY')),
    PRIMARY KEY (project_id, goal_id)
);

-- =====================================================================
-- Task layer
-- =====================================================================
CREATE TABLE tasks (
    id                  TEXT PRIMARY KEY,
    project_id          TEXT REFERENCES projects(id) ON DELETE SET NULL,
    goal_id             TEXT REFERENCES goals(id) ON DELETE SET NULL,
    title               TEXT NOT NULL,
    description         TEXT,
    status              TEXT NOT NULL DEFAULT 'INBOX'
        CHECK (status IN ('INBOX','TODO','SCHEDULED','IN_PROGRESS','PAUSED',
                          'COMPLETED','CANCELLED')),
    plan_date           INTEGER,                              -- 'YYYY-MM-DD' as INTEGER
    deadline            INTEGER,                              -- epoch ms
    priority            INTEGER NOT NULL DEFAULT 0,           -- 0..100
    importance          INTEGER NOT NULL DEFAULT 0,           -- 0..100 (human-set)
    estimated_minutes   INTEGER NOT NULL DEFAULT 30
        CHECK (estimated_minutes > 0),
    actual_minutes      INTEGER NOT NULL DEFAULT 0,
    is_splittable       INTEGER NOT NULL DEFAULT 0,
    requires_deep_work  INTEGER NOT NULL DEFAULT 0,
    energy_level        TEXT CHECK (energy_level IN ('LOW','MEDIUM','HIGH')),
    context_tag         TEXT,                                 -- 'home','ide','no-internet',...
    postponement_count  INTEGER NOT NULL DEFAULT 0,
    completed_at        INTEGER,
    created_at          INTEGER NOT NULL,
    updated_at          INTEGER NOT NULL,
    archived            INTEGER NOT NULL DEFAULT 0,
    deleted_at          INTEGER                               -- soft delete watermark
);

CREATE TABLE task_dependencies (
    id                 TEXT PRIMARY KEY,
    task_id            TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    depends_on_task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    dependency_type    TEXT NOT NULL DEFAULT 'FINISH_TO_START'
        CHECK (dependency_type IN ('FINISH_TO_START','START_TO_START')),
    UNIQUE (task_id, depends_on_task_id)
);

-- =====================================================================
-- Inbox (brain dump)
-- =====================================================================
CREATE TABLE inbox_items (
    id                                  TEXT PRIMARY KEY,
    raw_text                            TEXT NOT NULL,
    kind                                TEXT NOT NULL DEFAULT 'UNKNOWN'
        CHECK (kind IN ('UNKNOWN','TASK','IDEA','PROJECT','RESPONSIBILITY',
                        'OBLIGATION','RESEARCH','EXPERIMENT','AMBITION','NOTE')),
    status                              TEXT NOT NULL DEFAULT 'RAW'
        CHECK (status IN ('RAW','CLASSIFYING','CLASSIFIED','PROPOSED',
                          'CONVERTED','DISMISSED','ARCHIVED')),
    captured_at                         INTEGER NOT NULL,
    source                              TEXT NOT NULL DEFAULT 'manual'
        CHECK (source IN ('manual','quick_capture','shortcut','focus_overlay',
                          'tray','import')),
    classification                      TEXT,                 -- latest committed snapshot
    classification_proposed_at          INTEGER,
    classification_approved_at          INTEGER,
    project_id                          TEXT REFERENCES projects(id) ON DELETE SET NULL,
    goal_id                             TEXT REFERENCES goals(id) ON DELETE SET NULL,
    created_at                          INTEGER NOT NULL,
    updated_at                          INTEGER NOT NULL,
    archived                            INTEGER NOT NULL DEFAULT 0
);

-- Append-only AI suggestions (inspectable, independently ratable/approvable).
CREATE TABLE classification_proposals (
    id            TEXT PRIMARY KEY,
    inbox_item_id TEXT NOT NULL REFERENCES inbox_items(id) ON DELETE CASCADE,
    proposal      TEXT NOT NULL,   -- full classification JSON (kind, project ref,
                                   -- goal ref, duration, priority, deadline,
                                   -- actionability, next step, dependencies, ...)
    status        TEXT NOT NULL DEFAULT 'PENDING'
        CHECK (status IN ('PENDING','APPROVED','REJECTED','SUPERSEDED')),
    confidence    REAL NOT NULL DEFAULT 0 CHECK (confidence >= 0 AND confidence <= 1),
    created_at    INTEGER NOT NULL,
    decided_at    INTEGER,
    decided_by    TEXT CHECK (decided_by IN ('user','autonomy'))
);

-- =====================================================================
-- Daily hybrid plans (versioned)
-- =====================================================================
CREATE TABLE daily_plans (
    id                       TEXT PRIMARY KEY,
    plan_date                TEXT NOT NULL,                   -- 'YYYY-MM-DD' (local)
    date_start_utc           INTEGER,
    date_end_utc             INTEGER,
    main_objective           TEXT,
    status                   TEXT NOT NULL DEFAULT 'DRAFT'
        CHECK (status IN ('DRAFT','ACTIVE','COMPLETED','SUPERSEDED','REVIEWED')),
    version                  INTEGER NOT NULL DEFAULT 1,
    supersedes_plan_id       TEXT REFERENCES daily_plans(id),
    planning_mode            TEXT NOT NULL DEFAULT 'AUTO'
        CHECK (planning_mode IN ('AUTO','ADAPTIVE','MANUAL')),
    adaptive_mutations       INTEGER NOT NULL DEFAULT 0,      -- auto-applied change counter
    planned_work_minutes     INTEGER NOT NULL DEFAULT 0,
    completed_work_minutes   INTEGER NOT NULL DEFAULT 0,
    workload_reserve_minutes INTEGER NOT NULL DEFAULT 0,
    feasibility              TEXT,                            -- JSON feasibility/overflow report
    created_at               INTEGER NOT NULL,
    updated_at               INTEGER NOT NULL
);

-- Time-anchored blocks (deep work, commitments, breaks, buffers, unavailability).
CREATE TABLE scheduled_blocks (
    id               TEXT PRIMARY KEY,
    daily_plan_id    TEXT NOT NULL REFERENCES daily_plans(id) ON DELETE CASCADE,
    task_id          TEXT REFERENCES tasks(id) ON DELETE SET NULL,
    project_id       TEXT REFERENCES projects(id) ON DELETE SET NULL,
    goal_id          TEXT REFERENCES goals(id) ON DELETE SET NULL,
    block_type       TEXT NOT NULL
        CHECK (block_type IN ('DEEP_WORK','FOCUS_SESSION','MEETING',
                              'FIXED_COMMITMENT','BREAK','BUFFER','UNAVAILABLE')),
    scheduled_start  INTEGER NOT NULL,
    scheduled_end    INTEGER NOT NULL CHECK (scheduled_end > scheduled_start),
    actual_start     INTEGER,
    actual_end       INTEGER,
    duration_minutes INTEGER NOT NULL CHECK (duration_minutes > 0),
    status           TEXT NOT NULL DEFAULT 'PLANNED'
        CHECK (status IN ('PLANNED','ACTIVE','COMPLETED','SKIPPED',
                          'RESCHEDULED','CANCELLED')),
    priority         INTEGER NOT NULL DEFAULT 0,
    rationale        TEXT,                                    -- scheduling rationale
    source           TEXT NOT NULL DEFAULT 'PLANNER'
        CHECK (source IN ('PLANNER','ADAPTIVE','USER','IMPORT')),
    adaptive         INTEGER NOT NULL DEFAULT 0,
    created_at       INTEGER NOT NULL,
    updated_at       INTEGER NOT NULL
);

-- Flexible, priority-scored queue (NOT time-anchored).
CREATE TABLE flexible_queue_items (
    id                   TEXT PRIMARY KEY,
    daily_plan_id        TEXT NOT NULL REFERENCES daily_plans(id) ON DELETE CASCADE,
    task_id              TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    position             INTEGER NOT NULL,
    score                REAL NOT NULL CHECK (score >= 0 AND score <= 1),
    urgency              INTEGER NOT NULL DEFAULT 0,          -- 0..100
    strategic_alignment  REAL NOT NULL DEFAULT 0,             -- 0..1
    context_tag          TEXT,
    energy_level         TEXT CHECK (energy_level IN ('LOW','MEDIUM','HIGH')),
    estimated_minutes    INTEGER NOT NULL CHECK (estimated_minutes > 0),
    status               TEXT NOT NULL DEFAULT 'QUEUED'
        CHECK (status IN ('QUEUED','SELECTED','STARTED','COMPLETED','DEFERRED','REMOVED')),
    queued_at            INTEGER NOT NULL,
    completed_at         INTEGER,
    UNIQUE (daily_plan_id, task_id)
);

-- =====================================================================
-- Execution tracking & reviews
-- =====================================================================
CREATE TABLE task_sessions (
    id                  TEXT PRIMARY KEY,
    task_id             TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    scheduled_block_id  TEXT REFERENCES scheduled_blocks(id) ON DELETE SET NULL,
    start_time          INTEGER NOT NULL,
    end_time            INTEGER,
    duration_minutes    INTEGER NOT NULL DEFAULT 0,
    session_type        TEXT NOT NULL DEFAULT 'FOCUS'
        CHECK (session_type IN ('FOCUS','POMODORO')),
    pomodoro_rounds     INTEGER NOT NULL DEFAULT 0,
    notes               TEXT,
    created_at          INTEGER NOT NULL,
    updated_at          INTEGER NOT NULL
);

CREATE TABLE daily_reviews (
    id                TEXT PRIMARY KEY,
    daily_plan_id     TEXT NOT NULL REFERENCES daily_plans(id) ON DELETE CASCADE,
    review_date       TEXT NOT NULL,
    status            TEXT NOT NULL DEFAULT 'DRAFT'
        CHECK (status IN ('DRAFT','COMPLETE')),
    what_went_well    TEXT,
    blockers          TEXT,
    what_to_change    TEXT,
    insights          TEXT,                   -- AI-extracted insights (Zod-validated JSON)
    planning_accuracy REAL,                   -- 0..1
    ai_generated_at   INTEGER,
    ai_approved_at    INTEGER,
    created_at        INTEGER NOT NULL,
    updated_at        INTEGER NOT NULL
);

CREATE TABLE quarterly_reviews (
    id                TEXT PRIMARY KEY,
    quarter_key       TEXT NOT NULL,          -- 'YYYY-Qn'
    start_date        INTEGER,
    end_date          INTEGER,
    summary           TEXT,
    findings          TEXT,                   -- JSON: goals/milestones breakdown, lessons
    recommendations   TEXT,                   -- JSON: next-quarter recommendations
    status            TEXT NOT NULL DEFAULT 'DRAFT'
        CHECK (status IN ('DRAFT','COMPLETE')),
    ai_generated_at   INTEGER,
    ai_approved_at    INTEGER,
    created_at        INTEGER NOT NULL,
    updated_at        INTEGER NOT NULL,
    UNIQUE (quarter_key)
);

-- =====================================================================
-- Event log: append-only, reversible operations
-- =====================================================================
CREATE TABLE schedule_events (
    id             TEXT PRIMARY KEY,
    daily_plan_id  TEXT REFERENCES daily_plans(id) ON DELETE SET NULL,
    occurred_at    INTEGER NOT NULL,
    event_type     TEXT NOT NULL,      -- BLOCK_CREATED / BLOCK_MOVED / BLOCK_CANCELLED /
                                       -- TASK_ADDED / QUEUE_REORDERED / PLAN_REPLANNED /
                                       -- PLAN_RESTORED / ESTIMATE_UPDATED ...
    actor          TEXT NOT NULL
        CHECK (actor IN ('USER','AI','AUTONOMY','SCHEDULER','SYSTEM')),
    subject_id     TEXT,               -- referenced entity UUID
    before_payload TEXT,               -- JSON previous state
    after_payload  TEXT,               -- JSON resulting state
    undo_payload   TEXT,               -- JSON inverse-op descriptor
    reverted       INTEGER NOT NULL DEFAULT 0,
    created_at     INTEGER NOT NULL
);

-- =====================================================================
-- Adaptive autonomy: audit trail & approvals
-- =====================================================================
CREATE TABLE autonomy_decisions (
    id                  TEXT PRIMARY KEY,
    action_id           TEXT NOT NULL UNIQUE,
    action_type         TEXT NOT NULL,
    occurred_at         INTEGER NOT NULL,
    trigger             TEXT,           -- 'task_completed' | 'new_urgent_inbox' |
                                        -- 'block_overrun' | 'skipped_task' | ...
    level               TEXT NOT NULL
        CHECK (level IN ('AUTO_EXECUTE','PROPOSE','REQUIRE_CONFIRMATION')),
    policy              TEXT NOT NULL,  -- matched policy rules snapshot (JSON)
    status              TEXT NOT NULL
        CHECK (status IN ('EXECUTED','PROPOSED','APPROVED','REJECTED',
                          'CONFIRMED','WITHDRAWN')),
    reason              TEXT,
    before_payload      TEXT,           -- JSON
    proposed_payload    TEXT,           -- JSON
    result_payload      TEXT,           -- JSON
    related_task_ids    TEXT,           -- JSON array
    approval_request_id TEXT REFERENCES approval_requests(id) ON DELETE SET NULL,
    undo_event_id       TEXT REFERENCES schedule_events(id) ON DELETE SET NULL,
    created_at          INTEGER NOT NULL,
    updated_at          INTEGER NOT NULL
);

CREATE TABLE approval_requests (
    id                   TEXT PRIMARY KEY,
    autonomy_decision_id TEXT NOT NULL REFERENCES autonomy_decisions(id) ON DELETE CASCADE,
    title                TEXT NOT NULL,
    description          TEXT,
    impact_summary       TEXT,          -- JSON: what changes, what is displaced
    status               TEXT NOT NULL DEFAULT 'PENDING'
        CHECK (status IN ('PENDING','APPROVED','REJECTED','EXPIRED')),
    requested_at         INTEGER NOT NULL,
    decided_at           INTEGER,
    decided_by           TEXT CHECK (decided_by IN ('user','autonomy'))
);

-- =====================================================================
-- AI planning history (diagnostic / non-critical)
-- =====================================================================
CREATE TABLE ai_planning_history (
    id                     TEXT PRIMARY KEY,
    operation              TEXT NOT NULL,   -- classify_inbox | plan_daily | decompose_goal |
                                            -- schedule_explain | review_daily | review_quarterly
    status                 TEXT NOT NULL
        CHECK (status IN ('SUCCESS','FAILED','VALIDATION_ERROR','OVERRIDDEN','SKIPPED')),
    model                  TEXT,
    prompt_tokens          INTEGER,
    completion_tokens      INTEGER,
    context_payload_hash   TEXT,
    sent_at                INTEGER,
    completed_at           INTEGER,
    raw_response           TEXT,            -- truncated by the writer to a byte cap
    validated_output       TEXT,            -- sanitized/approved output JSON
    error                  TEXT,
    created_at             INTEGER NOT NULL
);

-- =====================================================================
-- Notifications (reminder engine persists future deliverables here)
-- =====================================================================
CREATE TABLE notifications (
    id            TEXT PRIMARY KEY,
    kind          TEXT NOT NULL
        CHECK (kind IN ('REMINDER_30M','REMINDER_START','OVERDUE','DAILY_PLANNING',
                        'DAILY_REVIEW','SCHEDULE_CHANGE','FOCUS','SYSTEM')),
    scheduled_for INTEGER NOT NULL,
    sent_at       INTEGER,
    status        TEXT NOT NULL DEFAULT 'PENDING'
        CHECK (status IN ('PENDING','SENT','SUPPRESSED','MISSED','DISMISSED')),
    payload       TEXT,                      -- JSON (block_id, task_id, message)
    created_at    INTEGER NOT NULL
);

-- =====================================================================
-- Scheduler runtime state (persistent, resumable across restarts)
-- =====================================================================
CREATE TABLE scheduler_state (
    key         TEXT PRIMARY KEY,            -- 'watermark' | 'last_plan_generation' | 'guards'
    value       TEXT NOT NULL                -- JSON
);

-- =====================================================================
-- Indexes
-- =====================================================================
CREATE INDEX idx_tasks_status          ON tasks(status) WHERE deleted_at IS NULL;
CREATE INDEX idx_tasks_project         ON tasks(project_id);
CREATE INDEX idx_tasks_goal            ON tasks(goal_id);
CREATE INDEX idx_tasks_plan_date       ON tasks(plan_date);
CREATE INDEX idx_tasks_deadline        ON tasks(deadline);
CREATE INDEX idx_projects_status       ON projects(status);
CREATE INDEX idx_milestones_goal       ON milestones(goal_id);
CREATE INDEX idx_inbox_status_captured ON inbox_items(status, captured_at);
CREATE INDEX idx_proposals_item        ON classification_proposals(inbox_item_id, status);
CREATE INDEX idx_blocks_plan_start     ON scheduled_blocks(daily_plan_id, scheduled_start);
CREATE INDEX idx_queue_plan_position   ON flexible_queue_items(daily_plan_id, position);
CREATE INDEX idx_sessions_task_start   ON task_sessions(task_id, start_time);
CREATE INDEX idx_events_plan_occurred  ON schedule_events(daily_plan_id, occurred_at);
CREATE INDEX idx_autonomy_occurred     ON autonomy_decisions(occurred_at);
CREATE INDEX idx_autonomy_status       ON autonomy_decisions(status);
CREATE INDEX idx_approval_status       ON approval_requests(status);
CREATE INDEX idx_notifications_sched   ON notifications(status, scheduled_for);

-- =====================================================================
-- Migration v0002 — goal Sidecar observations (separate from goal progress)
-- =====================================================================
CREATE TABLE goal_sidecar_entries (
    id               TEXT PRIMARY KEY,
    goal_id          TEXT NOT NULL REFERENCES goals(id) ON DELETE CASCADE,
    source            TEXT NOT NULL
        CHECK (source IN ('USER','FOCUS_OS','INTEGRATION','SYSTEM','AI')),
    entry_type        TEXT NOT NULL
        CHECK (entry_type IN ('CHECK_IN','ACTIVITY','ASSESSMENT')),
    content           TEXT NOT NULL,
    application_name  TEXT,
    window_title      TEXT,
    domain            TEXT,
    observed_start    INTEGER,
    observed_end      INTEGER,
    duration_seconds  INTEGER CHECK (duration_seconds IS NULL OR duration_seconds >= 0),
    confidence        REAL NOT NULL DEFAULT 1
        CHECK (confidence >= 0 AND confidence <= 1),
    review_status     TEXT NOT NULL DEFAULT 'CONFIRMED'
        CHECK (review_status IN ('CONFIRMED','PENDING','REJECTED')),
    source_ref        TEXT,
    created_at        INTEGER NOT NULL,
    updated_at        INTEGER NOT NULL,
    CHECK (observed_start IS NULL OR observed_end IS NULL OR observed_end >= observed_start),
    UNIQUE (source, source_ref)
);

CREATE INDEX idx_goal_sidecar_goal_time
    ON goal_sidecar_entries(goal_id, created_at DESC);
CREATE INDEX idx_goal_sidecar_source_time
    ON goal_sidecar_entries(source, created_at DESC);

-- FTS5 virtual table (added in a later migration) for AI context selection:
-- CREATE VIRTUAL TABLE fts_context USING fts5(
--     task_title, task_desc, project_name, inbox_text, content='');