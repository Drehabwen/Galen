# Galen SearchRun / Evidence Coverage Ledger Handoff

Date: 2026-08-31 (Asia/Shanghai)

## Mission

Continue the next product cut: make every literature search observable and durable, derive truthful provider coverage, inject that coverage into model context, and show it in the existing Evidence/Session Inspector.

Do not report this feature as complete until PubMed and recognized literature MCP calls actually write terminal SearchRun records and the frontend distinguishes searched-zero, failed, unavailable, disabled, connected-not-searched, and not-configured.

## Repository state

- Repository: `D:\DEV\Galen-new`
- Branch: `galen-research-workbench`
- The user previously requested working in the current checkout without a worktree.
- Do not modify, delete, stage, or commit these pre-existing untracked directories:

```text
output/e2e-artifact-loop-v2/
output/real-task-evidence/
```

Relevant commits, newest first:

```text
3face79 fix(galen): harden search run ledger boundaries
c8a9555 feat(galen): add durable literature search ledger
828103d docs(galen): plan literature coverage ledger
2c49a32 refactor(galen): retire discussion mode
1e6a981 docs(galen): add literature MCP handoff
7f47dc1 feat(galen): enable CNKI literature provider
fc636f7 feat(galen): enable built-in literature MCP providers
ed99699 docs(galen): design MCP literature gateway
```

## Source documents

- Binding design: `docs/superpowers/specs/2026-08-31-mcp-literature-gateway-design.md`
- Implementation plan: `docs/superpowers/plans/2026-08-31-search-run-coverage-ledger.md`
- SDD scratch/ledger: `.superpowers/sdd/2026-08-31-search-run-coverage-ledger/`
- Task 2 reconnaissance: `.superpowers/sdd/2026-08-31-search-run-coverage-ledger/task-2-recon.md`
- Task 3/4 reconnaissance: `.superpowers/sdd/2026-08-31-search-run-coverage-ledger/task-3-4-recon.md`

## Completed and committed

### Discussion-mode retirement

The product UI now exposes only `auto` and `plan`. Legacy `discuss` settings normalize to `auto`. This is committed in `2c49a32` and previously verified with:

```text
cargo test -p galen --lib: 148/148 passed
npm test -- --run: 34/34 passed
npm run build: passed
```

### SearchRun core through fix round 1

`rust/crates/galen/src-tauri/src/search_run.rs` and module registration are committed through `3face79`.

Committed behavior includes:

- workspace/task-scoped append-only `search-runs.jsonl`;
- SearchRun terminal statuses;
- six provider coverage states;
- successful zero results remain `Succeeded` with `result_count = 0`;
- coverage remembers successful history even when the latest attempt failed;
- UUID v4 run IDs;
- caller-supplied start and finish timestamps;
- strict 64-hex SHA-256 digest newtype;
- raw provider response is not persisted;
- raw errors are reduced to a safe error class;
- coverage serialization does not embed full SearchRun records;
- process-wide mutex plus one-buffer `write_all` for JSONL append safety;
- task/path validation and migration-safe loading.

Last verified committed state at `3face79`:

```text
cargo test -p galen search_run -- --nocapture: 12/12 passed
cargo test -p galen --lib: 160/160 passed
```

The reviewer accepted the above five fixes: secret isolation, strict digest validation, real timing, process-local concurrency safety, and stronger IDs.

## Current uncommitted work — do not discard

There is one modified tracked file:

```text
M rust/crates/galen/src-tauri/src/search_run.rs
```

This is an interrupted Task 1 fix-round-2 implementation. It replaces the too-lossy argument-key allowlist with a recursively redacted, structure-preserving projection.

The working diff currently adds approximately 284 lines and removes 53 lines. It includes:

- `MAX_ARGUMENT_DEPTH = 12`;
- `MAX_ARGUMENT_BYTES = 32 * 1024`;
- recursive preservation of objects, arrays, numbers, booleans, nulls, and legitimate strings;
- sensitive-key normalization and redaction;
- redaction of credential-, cookie-, environment-, browser-profile-, and absolute-path-like values;
- support for `{ field/name/key/type: <sensitive>, value: ... }` shapes;
- explicit `[truncated]` sentinels for depth/size limits;
- new tests beginning around `search_run.rs:870` for unknown nested provider arguments, nested secret redaction, limits, and existing leakage boundaries.

The implementing agent was interrupted before reporting final tests or committing. Therefore:

> Do not assume the working diff compiles or passes. Do not commit it unchanged without inspection and fresh verification.

## Why fix round 2 exists

The first security fix used a hard-coded 21-key allowlist. Independent review rejected it because the design requires original search arguments for reproducibility, while the allowlist silently discarded legitimate Crossref, Semantic Scholar, and CNKI provider-specific fields and all nested objects.

The required balance is:

```text
preserve original argument structure
        +
recursively redact secrets and sensitive paths
        +
apply bounded depth/size
```

Do not return to a small global allowlist. If provider-specific schemas are used instead, they must cover every recognized tool without silently dropping legitimate fields.

## Immediate next steps

1. Inspect the current `search_run.rs` working diff rather than overwriting it.
2. Run formatting only on the touched file/crate if necessary; the repository previously had unrelated global formatting differences.
3. Run focused tests:

```powershell
cd D:\DEV\Galen-new\rust
cargo test -p galen search_run -- --nocapture
```

4. Review the new projection carefully for:
   - preserved unknown legitimate nested fields;
   - no leaked token/password/cookie/API key/authorization/env/profile/path values;
   - deterministic depth/size behavior;
   - valid JSON after truncation;
   - no sensitive data in serialized `ProviderCoverage`.
5. Run the full Galen backend suite:

```powershell
cargo test -p galen --lib
```

6. Commit only after those tests pass, using a message such as:

```text
fix(galen): preserve redacted search arguments
```

7. Re-review only the open provenance finding. The required verdict is that legitimate nested provider arguments survive while secrets are redacted.

## Remaining implementation tasks

### Task 2 — automatic recording at the real search boundary

Not started.

Recommended integration from reconnaissance:

- wrap the common result/error boundary in `ToolRegistry::execute_dynamic`;
- recognize searches from a declarative provider/tool catalog;
- use resolved `(server_name, tool_name)` for MCP, never arbitrary output text;
- preserve the exact existing tool result/error returned to the model;
- append exactly one terminal SearchRun per recognized invocation;
- record failures before returning the error;
- for built-in PubMed/Rehab search, use `papers.len()` as the reliable count for this slice;
- do not record unrelated MCP tools such as login/download/read-online operations as searches.

Primary files:

```text
rust/crates/galen/src-tauri/src/tools/mod.rs
rust/crates/galen/src-tauri/src/tools/medical.rs
rust/crates/galen/src-tauri/src/mcp_client.rs
rust/crates/galen/src-tauri/src/chat_loop.rs
```

### Task 3 — secret-free coverage command and model boundary

Not started.

- add `get_literature_coverage` using the host-selected workspace and active task;
- never accept an arbitrary WebView workspace path as authority;
- never serialize MCP command, args, environment, raw output, cookies, or full raw errors;
- inject a compact coverage section into synthesis context;
- force qualified wording when any relevant source is unsearched/failed/unavailable;
- explicitly distinguish `searched (0 results)` from failure;
- CNKI failure/unavailability must say it is a coverage limitation, never “no Chinese evidence.”

Primary files:

```text
rust/crates/galen/src-tauri/src/commands.rs
rust/crates/galen/src-tauri/src/lib.rs
rust/crates/galen/src-tauri/src/context_engine.rs
rust/crates/galen/src-tauri/src/context_engine_tests.rs
```

### Task 4 — Evidence Coverage card

Not started.

- add a hook calling the coverage command;
- render a compact expandable card inside the existing `SessionInspectorDrawer`;
- do not add another page, navigation mode, or rail tab;
- refresh on workspace, active task, and completed-chat changes;
- show query details only on expansion;
- show searched-zero, failed, disabled, unavailable, connected-not-searched, and not-configured distinctly;
- use `src/styles/layout.css`, because the planned `workbench.css` does not exist in the current checkout.

Primary files:

```text
rust/crates/galen/src/hooks/useLiteratureCoverage.ts
rust/crates/galen/src/components/EvidenceCoverageCard.tsx
rust/crates/galen/src/components/EvidenceCoverageCard.test.tsx
rust/crates/galen/src/components/SessionInspectorDrawer.tsx
rust/crates/galen/src/App.tsx
rust/crates/galen/src/styles/layout.css
```

## CNKI status

CNKI work is intentionally paused at the user's request.

- MCP package is installed and Galen discovers nine CNKI tools.
- A real CNKI search has not succeeded.
- iKuuuVPN intercepts CNKI DNS into `198.18.0.x`; direct testing reached certificate interference / Tencent EdgeOne HTTP 418.
- Do not claim CNKI was searched.
- Do not block PubMed/Crossref/SearchRun coverage work on CNKI.

The earlier detailed CNKI handoff remains at:

```text
docs/handoffs/2026-08-31-literature-mcp-cnki-handoff.md
```

## Verification and completion rules

Before claiming this feature complete, verify freshly:

```powershell
cd D:\DEV\Galen-new\rust
cargo test -p galen --lib

cd D:\DEV\Galen-new\rust\crates\galen
npm test -- --run
npm run build
```

Acceptance requires a deterministic test or smoke path proving:

1. a successful PubMed zero-result call creates a succeeded SearchRun with count zero;
2. a recognized MCP failure creates a failed SearchRun without leaking raw error or secrets;
3. restarting/loading restores SearchRun history for the active task;
4. model context lists searched and limited providers before synthesis;
5. the inspector displays the same truthful coverage states;
6. CNKI failed/unavailable never becomes a “no Chinese evidence” conclusion.

## Operational note

During Task 1, `D:\DEV\Galen-new\rust\target` had grown to about 64.1 GiB and caused Windows error 112. The implementer verified that exact reproducible build-output path and ran `cargo clean`; subsequent committed-state tests passed. If disk pressure returns, clean only verified generated build output, never workspace data or the untracked output directories named above.
