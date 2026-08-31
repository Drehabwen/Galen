# SearchRun Coverage Ledger Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Persist every literature search and expose provider coverage so Galen cannot confuse unsearched, unavailable, failed, or successful-zero-result sources.

**Architecture:** Add a workspace/task-scoped append-only `SearchRun` ledger in the Tauri backend. Recognized built-in and MCP literature searches append terminal records, coverage is derived from provider configuration plus the latest run, and the compact result is exposed to both model context and the existing evidence inspector UI.

**Tech Stack:** Rust, serde/serde_json, Tauri commands, React/TypeScript, Vitest.

**Spec:** `docs/superpowers/specs/2026-08-31-mcp-literature-gateway-design.md`

## Global Constraints

- A successful search with zero results is `searched` with `result_count = 0`, never `failed` or `unsearched`.
- Failed, unavailable, disabled, connected-not-searched, and not-configured providers remain distinct.
- Store a SHA-256 content hash, never the full raw provider response.
- SearchRun is retrieval provenance; it does not automatically create claim evidence.
- Provider credentials, environment values, cookies, and browser profiles must never enter the ledger or frontend response.
- CNKI failure must produce a coverage limitation, not a claim that Chinese evidence does not exist.

---

### Task 1: Durable SearchRun ledger and coverage derivation

**Files:**
- Create: `rust/crates/galen/src-tauri/src/search_run.rs`
- Modify: `rust/crates/galen/src-tauri/src/lib.rs`
- Test: inline unit tests in `search_run.rs`

**Interfaces:**
- Produces: `SearchRun`, `SearchRunStatus`, `CoverageState`, `ProviderCoverage`, `append_search_run`, `load_search_runs`, and `derive_coverage`.
- Consumes: workspace root, active task ID, configured provider descriptors, and terminal search records.

- [ ] **Step 1: Write failing serialization and persistence tests**

```rust
#[test]
fn append_and_load_preserves_zero_result_success() {
    let run = SearchRun::succeeded("task-1", "pubmed", "search_pubmed", "stroke", 0, "abc");
    append_search_run(&root, &run).unwrap();
    let loaded = load_search_runs(&root, "task-1").unwrap();
    assert_eq!(loaded[0].status, SearchRunStatus::Succeeded);
    assert_eq!(loaded[0].result_count, Some(0));
}

#[test]
fn coverage_uses_latest_attempt_without_erasing_prior_search_history() {
    let coverage = derive_coverage(&providers, &[success, later_failure]);
    assert_eq!(coverage["pubmed"].state, CoverageState::Failed);
    assert!(coverage["pubmed"].has_successful_history);
}
```

- [ ] **Step 2: Run `cargo test -p galen search_run -- --nocapture` and verify failure because the module/types do not exist**
- [ ] **Step 3: Implement JSONL append/load with task-ID path validation, stable IDs, timestamps, status, count, bounded error, arguments, and raw-result hash**
- [ ] **Step 4: Implement deterministic coverage derivation for all six states and rerun the focused tests**
- [ ] **Step 5: Commit `feat(galen): add durable literature search ledger`**

### Task 2: Record built-in PubMed and recognized MCP searches

**Files:**
- Modify: `rust/crates/galen/src-tauri/src/tools/mod.rs`
- Modify: `rust/crates/galen/src-tauri/src/tools/research.rs`
- Modify: `rust/crates/galen/src-tauri/src/mcp_client.rs`
- Modify: `rust/crates/galen/src-tauri/src/chat_loop.rs`
- Test: focused tests in the modified Rust modules

**Interfaces:**
- Consumes: `append_search_run`, active task ID, tool name, arguments, result/error, recognized provider catalog.
- Produces: exactly one terminal SearchRun per recognized search invocation.

- [ ] **Step 1: Write failing tests proving PubMed success, MCP failure, and MCP zero-result success each append exactly one terminal record**

```rust
assert_eq!(load_search_runs(&root, "task-1").unwrap().len(), 1);
assert_eq!(runs[0].provider_id, "crossref");
assert_eq!(runs[0].status, SearchRunStatus::Failed);
```

- [ ] **Step 2: Run the focused tests and verify no records are currently written**
- [ ] **Step 3: Add a declarative recognized-search catalog for PubMed, Crossref, Semantic Scholar, and CNKI tool names**
- [ ] **Step 4: Wrap terminal tool outcomes, count structured results when available, hash raw output, append before returning output/error, and keep non-search MCP tools unrecorded**
- [ ] **Step 5: Rerun focused tests and `cargo test -p galen --lib`**
- [ ] **Step 6: Commit `feat(galen): record literature tool search runs`**

### Task 3: Coverage command and model-context boundary

**Files:**
- Modify: `rust/crates/galen/src-tauri/src/commands.rs`
- Modify: `rust/crates/galen/src-tauri/src/lib.rs`
- Modify: `rust/crates/galen/src-tauri/src/context_engine.rs`
- Test: `rust/crates/galen/src-tauri/src/context_engine_tests.rs`

**Interfaces:**
- Produces: Tauri command `get_literature_coverage(workspace_root)` and compact model context section `## Literature coverage`.
- Consumes: active task, provider configuration/status, and SearchRun ledger.

- [ ] **Step 1: Write failing command and prompt tests**

```rust
assert!(prompt.contains("PubMed: searched (0 results)"));
assert!(prompt.contains("CNKI: failed; do not infer absence of Chinese evidence"));
```

- [ ] **Step 2: Verify the focused tests fail because coverage is absent from the command and prompt**
- [ ] **Step 3: Implement a secret-free serialized coverage response and compact prompt renderer**
- [ ] **Step 4: Require final claims to use “based on searched providers” language whenever any configured source is not successfully searched**
- [ ] **Step 5: Rerun focused tests and the full Galen backend suite**
- [ ] **Step 6: Commit `feat(galen): enforce literature coverage boundaries`**

### Task 4: Evidence Coverage card in the existing inspector

**Files:**
- Create: `rust/crates/galen/src/hooks/useLiteratureCoverage.ts`
- Create: `rust/crates/galen/src/components/EvidenceCoverageCard.tsx`
- Create: `rust/crates/galen/src/components/EvidenceCoverageCard.test.tsx`
- Modify: `rust/crates/galen/src/components/SessionInspectorDrawer.tsx`
- Modify: `rust/crates/galen/src/styles/workbench.css`

**Interfaces:**
- Consumes: `get_literature_coverage` response and current workspace/task refresh signals.
- Produces: compact provider rows with state, count, time, query disclosure, and a visible overall limitation.

- [ ] **Step 1: Write failing component tests for searched-zero, failed, disabled, unavailable, and never-searched labels**

```tsx
expect(screen.getByText("已检索 · 0 条")).toBeInTheDocument();
expect(screen.getByText("失败 · 不代表没有中文证据")).toBeInTheDocument();
```

- [ ] **Step 2: Run `npm test -- EvidenceCoverageCard.test.tsx` and verify the missing component failure**
- [ ] **Step 3: Implement the hook and compact card without adding a new navigation mode or page**
- [ ] **Step 4: Render query details only on expansion and never render provider environment values**
- [ ] **Step 5: Run `npm test -- --run`, `npm run build`, and `cargo test -p galen --lib`**
- [ ] **Step 6: Commit `feat(galen): show literature source coverage`**

### Final acceptance

- [ ] A PubMed zero-result search is visibly successful with count zero.
- [ ] A Crossref or Semantic Scholar failure is visibly failed and retains its error class.
- [ ] CNKI unavailable/failed never produces “no Chinese evidence.”
- [ ] Restarting Galen restores SearchRun history for the active task.
- [ ] The model context lists searched and unsearched/failed providers before synthesis.
- [ ] Backend tests, frontend tests, TypeScript, and production build pass.
