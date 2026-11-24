# ADR 0003 — Read-only GitHub portfolio connector

- **Status:** Accepted
- **Date:** 2026-11-13

## Context

The user wants help turning real project work into portfolio evidence and a
CV draft. The obvious source is their own GitHub repositories, but a personal
operating system must not become a write-capable integration or quietly ship
source code to a remote model.

The GitHub connector was added ahead of the integration phase because it only
needs read access and does not touch scheduling, goals, or autonomy state.

## Decision

- The connector uses a **fine-grained, read-only** GitHub token. The token is
  stored in the OS keyring, never in `app_settings` and never in the frontend.
- Only repositories **owned by the authenticated account** are listed. The
  token's scopes are not widened by the application.
- Repository inspection happens **locally**: the connector fetches the tree and
  a bounded set of text files, then derives a deterministic summary (file
  counts, languages, manifest files, declaration counts, README presence).
  Generated/vendor/dependency files are excluded from the metrics.
- The **raw source never leaves the device**. Only the displayed derived
  summary is sent to NVIDIA NIM, and only after a separate explicit user
  confirmation on the Settings screen.
- The assessment and the editable CV draft are stored locally in
  `portfolio_assessments`. The draft is user-editable; nothing is published,
  committed, or sent anywhere else.
- AI-generated claims are kept separate from verification notes so the user can
  review every statement before using it.
- Removing the token deletes it from the keyring; no derived assessment is
  silently deleted.

## Consequences

- The integration cannot modify repositories, commits, issues, or profiles.
- The connector is useful offline for listing local-derived metrics; AI
  evaluation degrades cleanly when NIM is unavailable.
- A future write-capable integration would require a new ADR and a separate
  consent flow; it cannot reuse this token contract.
- Network failures, rate limits, and oversized files are bounded by the client;
  inspection reports what it skipped instead of guessing.
