# ADR 0002 — Goal Sidecar observability

- **Status:** Accepted
- **Date:** 2026-11-12

## Context

Official strategic objectives should remain stable and user-owned. The user
wants a visible, parallel measure of whether real activity is aligned with
those objectives, with AI help interpreting the evidence.

The user selected all three observation sources:

1. Focus OS task and focus-session activity.
2. Explicitly connected external integrations.
3. System activity, including application names, window titles, and web domains.

## Decision

- Store observations in a Sidecar stream linked to a goal, separate from the
  goal's official status and progress. Observation capture never mutates goals.
- Show the Sidecar beside each goal, with source, time, duration, and available
  activity details visible. AI assessments are identified as AI-generated and
  remain reviewable.
- Sidecar activity is local-first. System monitoring and external integrations
  are not enabled implicitly: each source requires an explicit setting/consent,
  visible pause/disable controls, and a way to inspect and remove collected
  Sidecar records.
- When enabled, system observation may retain application name, window title,
  domain, and duration. It must not collect keystrokes, document contents,
  screenshots, or full URLs/query strings.
- Do not transmit raw system activity details to a remote AI provider by
  default. The AI receives a minimized, goal-relevant summary unless the user
  separately authorizes more.
- An AI assessment is an interpretation, not a fact. Keep its confidence,
  provenance, and review status; never use it to automatically change an
  objective, declare success, or claim that a goal is achieved.
- Focus OS activity events and integration events are deduplicated by
  source-reference and are retained independently from the source application.

## Consequences

- The Sidecar uses an additive migration and can be deployed without rewriting
  existing goal data.
- The UI must distinguish official progress from observed activity and AI
  interpretation.
- OS monitoring and integrations require separate source adapters and controls;
  they are not considered active just because the Sidecar database exists.
- Privacy and retention settings must be implemented before activating
  system-level collection.
