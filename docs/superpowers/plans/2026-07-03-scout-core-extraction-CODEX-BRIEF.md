# Codex Brief: scout-core extraction

Executor: Codex (GPT-5.5). Author of record: Dominik.

## Job

Execute `docs/superpowers/plans/2026-07-03-scout-core-extraction.md` (this directory) task-by-task, Tasks 1→7, in order. The plan is the contract: exact paths, code, rename map, commands, expected outputs. Tick the plan's `- [ ]` checkboxes as you complete steps; commit plan-file updates with the task they belong to.

## Repos

- Source: `~/gitrepos/06_apps-utilities/01_desktop-apps/highlight-scout` — branch `scout-core-extraction` off `main` (baseline = tag `v0.5.5`).
- New: `~/gitrepos/06_apps-utilities/03_misc-utilities/scout-core` → GitHub `techczech/scout-core`, **private** (`gh repo create --private`).
- Task 7 touches `~/gitrepos/_COORDINATION` and `~/gitrepos/_REPOLOG` — push both the same turn.

## Hard rules

- Physical SQLite schema frozen: never rename tables/columns (`works`, `highlights`, `search_index`, `work_id`, `work_type`, `highlighted_at`). Generic naming is Rust-API-only. Never edit SQL strings during the rename.
- Dep versions pinned as written in the plan (`rusqlite =0.31.0` etc.). No new dependencies.
- Struct-literal field renames in importers: per-file hand edits, no blind sed.
- Git: no `--no-verify`, no force-push, no amends of pushed commits, commit lockfiles. Commit trailer: use `Co-Authored-By: GPT-5.5 Codex <noreply@openai.com>` in place of the Claude trailer shown in the plan's commit commands.
- Acceptance bar: highlight-scout behaviour parity — all test commands in the plan pass with the expected results; frontend wire contract unchanged.

## Checkpoints — stop and ask Dominik

1. After Task 4 (scout-core tagged v0.1.0, pushed): report before starting the highlight-scout switchover.
2. Task 6 Step 4 (live verification in the running app) requires Dominik at the keyboard. Do not skip; do not self-certify.
3. Any deviation from the plan (missing file, failing expected output, bun `prepare` fallback in Task 6 Step 1): stop, report what/why, wait. Do not redesign.

## Prerequisites (verify before Task 1)

- `gh auth status` OK for techczech.
- `ssh -T git@github.com` OK (cargo + bun git deps use ssh).
- `cargo`, `bun` on PATH; highlight-scout `main` clean and at/after v0.5.5.

## Completion report

Per repo: commits, tags, test counts before/after (plan predicts the arithmetic — verify against actual runs), deviations taken, clean-tree confirmation. Flag anything you'd revisit.
