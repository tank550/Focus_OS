# ADR 0001 — Note-driven project and task agent

- **Status:** Accepted
- **Date:** 2026-11-12

## Context

Focus OS is a brain-dump-first personal operating system. The user writes
unstructured notes; they should not have to decide in advance which thoughts
are projects, tasks, ideas, or responsibilities. The AI agent is responsible
for interpreting notes and creating the corresponding project and task records.

The user approved automatic creation of clear projects and tasks from notes.
This does not grant the model direct database access or permission to silently
change existing strategic data.

## Decision

- Preserve every captured note exactly as written. AI analysis never replaces
  or deletes its source note.
- The agent receives only the selected note and the minimum relevant local
  context. Processing requires an enabled, configured provider and a clear
  disclosure when note content will be sent to a remote provider.
- When AI is enabled and a provider is configured, a saved note is eligible for
  asynchronous organization. The raw note remains usable if analysis is
  unavailable; users can also request organization explicitly.
- Agent capabilities are a closed set of typed tools exposed by Rust
  application services, such as finding a matching project, creating a
  project, creating a task, and linking the source note. No SQL, arbitrary
  commands, or unrestricted filesystem/network tools are exposed to the model.
- Deterministic validation and autonomy policy run for every tool call. Clear,
  add-only task/project creation may execute automatically and is recorded in
  the autonomy audit trail. Ambiguous interpretations are surfaced for review.
- The agent does not automatically delete, archive, overwrite, or reprioritize
  existing records. Changes to goals, milestones, hard deadlines, fixed
  commitments, and major existing project state require explicit confirmation.
- Processing is idempotent per note revision. Re-running analysis must not
  create duplicate records; the raw note and its analysis/action history remain
  linked so generated records can be inspected and reversible actions undone.
- The Projects screen is a read/manage view for agent-created projects. Project
  creation belongs to the agent workflow, not a manual creation form.

## Consequences

- The first implementation increment is explicit, per-note analysis with a
  persisted proposal. An actionable task can be created only after explicit
  user confirmation, in one transaction with proposal approval and an audit
  decision. Project creation and automatic execution remain gated on the
  broader typed agent tools and autonomy policy below.
- `BrainDumpAnalyzer` must return multiple typed candidate actions, not just a
  single label for a note.
- Rust remains the only persistence boundary; model-provided IDs and references
  are validated against local records before use.
- Confidence alone is not authorization. Policy also considers ambiguity,
  whether an action is additive, and whether it affects existing strategic
  state.
- If no provider is configured or a response fails validation, the note remains
  intact and unprocessed; no partial entity creation is committed.
