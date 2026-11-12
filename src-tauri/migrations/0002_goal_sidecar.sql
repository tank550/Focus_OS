-- Goal Sidecar — observations kept separately from official goal state.
CREATE TABLE goal_sidecar_entries (
    id              TEXT PRIMARY KEY,
    goal_id         TEXT NOT NULL REFERENCES goals(id) ON DELETE CASCADE,
    source          TEXT NOT NULL
        CHECK (source IN ('USER','FOCUS_OS','INTEGRATION','SYSTEM','AI')),
    entry_type      TEXT NOT NULL
        CHECK (entry_type IN ('CHECK_IN','ACTIVITY','ASSESSMENT')),
    content         TEXT NOT NULL,
    application_name TEXT,
    window_title    TEXT,
    domain          TEXT,
    observed_start  INTEGER,
    observed_end    INTEGER,
    duration_seconds INTEGER CHECK (duration_seconds IS NULL OR duration_seconds >= 0),
    confidence      REAL NOT NULL DEFAULT 1
        CHECK (confidence >= 0 AND confidence <= 1),
    review_status   TEXT NOT NULL DEFAULT 'CONFIRMED'
        CHECK (review_status IN ('CONFIRMED','PENDING','REJECTED')),
    source_ref      TEXT,
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL,
    CHECK (observed_start IS NULL OR observed_end IS NULL OR observed_end >= observed_start),
    UNIQUE (source, source_ref)
);

CREATE INDEX idx_goal_sidecar_goal_time
    ON goal_sidecar_entries(goal_id, created_at DESC);
CREATE INDEX idx_goal_sidecar_source_time
    ON goal_sidecar_entries(source, created_at DESC);
