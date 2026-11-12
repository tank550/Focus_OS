# Phase 1 — Foundation (status notes)

Goal: Tauri 2 scaffold, SQLite pool + migrations, settings service, test
harness, container/design-system shell.

## Exit criteria / checklist

- [ ] App launches (`npm run tauri dev`) with the dark shell + sidebar nav
- [ ] Migrations apply cleanly on first run (`v0001` = full schema)
- [ ] `cargo test` green (domain + SQLite integration)
- [ ] `npm run typecheck` green (strict mode)
- [ ] `npm test` green (vitest)
- [ ] Empty account-free screens render for all six sections

## Delivered in this phase

- Workspace: `crates/focus-domain` (pure Rust, headless-testable) + `src-tauri`.
- Domain vocabulary (status enums) with a **SQL-contract test** pinning every
  literal against the DDL in `docs/SQLITE_SCHEMA.sql`.
- Migration runner: versioned, transactional, idempotent; `schema_migrations`
  bookkeeping. `0001_init.sql` is byte-identical to the docs DDL (hashes
  verified at copy time).
- Connection pool (`r2d2` + `rusqlite`, bundled SQLite, WAL, `foreign_keys=ON`,
  busy_timeout 5s).
- Typed IPC commands: `app_info`, `health`, `settings_get/set/all`.
- Frontend shell: sidebar navigation, six section screens, runtime status
  badge, Zustand stores (`appStore`, `settingsStore`), typed `bridge`.
- Integration tests: migrations idempotency, table presence, settings
  upsert, FK + CHECK violation typing, transaction rollback, WAL/FK pragmas.

## Environment notes

- Target workstation: Windows. **Rust would normally build with the MSVC
  toolchain.** During this session the shell lacked the MSVC linker (Visual
  Studio Build Tools need admin rights), so builds were verified with the
  `stable-x86_64-pc-windows-gnu` toolchain + `w64devkit` MinGW (user-scope,
  no admin). Commit-relevant CI must use MSVC on Windows.
- `src/lib/bridge.ts` + `src/shared/contracts.ts` are hand-written until
  `tauri-specta` codegen lands (Phase 2, aligns with the IPC contract rule).

## Technical debt (tracked)

- `tauri.conf.json` sets `"csp": null` — a strict CSP is required before
  packaging (Phase 9). Dev-mode asset loading currently needs no CSP.
- Command errors are surfaced as strings (`Result<_, String>`); typed error
  DTOs are scheduled with the Phase-2 contract codegen.
- `tauri-plugin-keyring` (OS credential storage) is Phase 7, per the roadmap.
- `count_tables()` asserts 22 tables after migration v0002 — update the
  constant when adding another application table.