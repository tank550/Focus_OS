CREATE TABLE portfolio_assessments (
    id              TEXT PRIMARY KEY,
    repo_full_name  TEXT NOT NULL,
    source_summary  TEXT NOT NULL,
    evaluation      TEXT NOT NULL,
    draft_markdown  TEXT NOT NULL,
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL
);

CREATE INDEX idx_portfolio_repo_time
    ON portfolio_assessments(repo_full_name, created_at DESC);
