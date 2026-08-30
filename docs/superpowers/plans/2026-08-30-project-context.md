# Galen Project Context Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a host-authoritative Project Context that remembers the current research direction, retires explicitly excluded directions, records evidence-source coverage, and makes PubMed searches update that durable state.

**Architecture:** Add one Rust domain service as the sole owner of `<workspace>/.galen/project-context.json`, expose it through narrow Tauri commands and a Research Pack tool, then inject its compact projection into every non-direct Agent turn. PubMed tools persist source coverage through the same service, while React renders the current question and coverage without owning canonical state.

**Tech Stack:** Rust 2021, Serde/serde_json, Tauri 2, React 18, TypeScript, Vitest, existing Galen eval CLI.

**Spec:** `docs/superpowers/specs/2026-08-30-project-context-design.md`

## Global Constraints

- Work only on `galen-research-workbench`; `main` is historical.
- Preserve all existing user changes and output directories in the primary checkout.
- Use an isolated Git worktree for implementation.
- Follow TDD: add one failing behavior test, observe the expected failure, then add minimal production code.
- Store canonical state only at `<workspace>/.galen/project-context.json` using temporary-file plus atomic replacement.
- Never store API keys, request headers, full provider responses, patient identifiers, or unde-identified clinical source material in Project Context.
- Replacing a direction clears the active task pointer but never deletes task, evidence, artifact, conversation, or decision history.
- CNKI, Wanfang, VIP, and CBM direct connectors and bibliographic import remain outside this implementation.
- Project Context outranks generic decision memory for research question, scope, excluded directions, and evidence coverage.
- A search request must execute an available search tool; a search expression alone is not a completed result.
- A global “no evidence” statement is forbidden unless every planned source is marked `searched`.
- Preserve the current 34 frontend and 136 Galen Rust tests.

---

## File Map

- Create `rust/crates/galen/src-tauri/src/project_context.rs`: domain types, validation, load/ensure/update, CAS persistence, rendering, and unit tests.
- Modify `rust/crates/galen/src-tauri/src/research_task.rs`: clear the active-task pointer without deleting task data.
- Create `rust/crates/galen/src-tauri/src/commands/project_context.rs`: Tauri query and mutation commands.
- Modify `rust/crates/galen/src-tauri/src/commands.rs`: declare the command submodule.
- Modify `rust/crates/galen/src-tauri/src/lib.rs`: export the domain module and register commands.
- Create `rust/crates/galen/src-tauri/src/tools/project_context.rs`: model-facing `update_project_context` tool.
- Modify `rust/crates/galen/src-tauri/src/tools/mod.rs`: declare the tool module.
- Modify `rust/crates/galen/src-tauri/src/capability.rs`: add the tool to Research Pack.
- Modify `rust/crates/galen/src-tauri/src/context_engine.rs`: inject the host-authoritative state and coverage guard.
- Modify `rust/crates/galen/src-tauri/src/context_engine_tests.rs`: test priority, continuity, replacement, and coverage language.
- Modify `rust/crates/galen/src-tauri/src/tools/medical.rs`: persist PubMed search lifecycle.
- Modify `rust/crates/galen/src/types.ts`: mirror Project Context types.
- Create `rust/crates/galen/src/hooks/useProjectContext.ts`: fetch, refresh, replace, and patch Project Context.
- Create `rust/crates/galen/src/components/ProjectContextStrip.tsx`: compact current-question and scope UI.
- Create `rust/crates/galen/src/components/EvidenceCoverageCard.tsx`: source coverage UI.
- Create `rust/crates/galen/src/components/ProjectContext.test.tsx`: component behavior tests.
- Modify `rust/crates/galen/src/App.tsx`: wire the hook and components without taking ownership of canonical state.
- Modify `rust/crates/galen/src/styles/workbench.css`: component styles using existing CSS variables.
- Create `evals/cases/e13_same_topic_continuity.toml` through `e18_workspace_isolation.toml`: regression contracts.
- Modify `evals/README.md`: document the Project Context evaluation slice.

---

### Task 1: Project Context domain store

**Files:**
- Create: `rust/crates/galen/src-tauri/src/project_context.rs`
- Modify: `rust/crates/galen/src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `research_task::load_or_migrate_active_task(&Path)`.
- Produces: `ProjectContext`, `ExcludedDirection`, `CoverageStatus`, `EvidenceSourceCoverage`, `load_project_context`, `ensure_project_context`, `replace_project_direction`, `patch_project_scope`, `update_evidence_coverage`, and `render_project_context`.

- [ ] **Step 1: Add failing load and default-source tests**

Add `pub mod project_context;` to `lib.rs`, create the module, and define tests first:

```rust
#[test]
fn empty_workspace_has_no_persisted_context() {
    let root = workspace("empty");
    assert_eq!(load_project_context(&root).unwrap(), None);
    assert!(!root.join(".galen/project-context.json").exists());
}

#[test]
fn ensure_initializes_known_source_coverage() {
    let root = workspace("defaults");
    let context = ensure_project_context(&root).unwrap();
    assert_eq!(context.revision, 1);
    assert_eq!(context.evidence_sources["pubmed"].status, CoverageStatus::NotSearched);
    assert_eq!(context.evidence_sources["cnki"].status, CoverageStatus::Unavailable);
}
```

- [ ] **Step 2: Run the focused tests and observe RED**

Run:

```powershell
cd rust
cargo test -p galen project_context::tests::empty_workspace_has_no_persisted_context -- --exact
```

Expected: compilation fails because the domain types and functions are not implemented.

- [ ] **Step 3: Implement types, validation, inference, and atomic persistence**

Implement the spec types with these exact public signatures:

```rust
pub const PROJECT_CONTEXT_SCHEMA_VERSION: u32 = 1;

pub fn load_project_context(workspace: &Path) -> Result<Option<ProjectContext>, String>;
pub fn ensure_project_context(workspace: &Path) -> Result<ProjectContext, String>;
pub fn replace_project_direction(
    workspace: &Path,
    expected_revision: u64,
    research_question: String,
    active_scope: Vec<String>,
    excluded_directions: Vec<String>,
    reason: String,
) -> Result<ProjectContext, String>;
pub fn patch_project_scope(
    workspace: &Path,
    expected_revision: u64,
    add: Vec<String>,
    remove: Vec<String>,
) -> Result<ProjectContext, String>;
pub fn update_evidence_coverage(
    workspace: &Path,
    expected_revision: u64,
    source: String,
    coverage: EvidenceSourceCoverage,
) -> Result<ProjectContext, String>;
pub fn render_project_context(context: &ProjectContext) -> String;
```

Use a module-local `Mutex<()>`, normalize scope with trim/dedup/first-order preservation, accept source keys matching `[a-z0-9][a-z0-9_-]{0,31}`, and persist through `project-context.json.pending` followed by `std::fs::rename`.

- [ ] **Step 4: Add failing mutation and safety tests**

```rust
#[test]
fn replacing_direction_retires_old_scope_and_resets_coverage() {
    let root = workspace("replace");
    let first = replace_project_direction(
        &root, 0, "中西医结合卒中康复".into(), vec!["中西医结合".into()], vec![], "首次确认".into()
    ).unwrap();
    let searched = update_evidence_coverage(
        &root,
        first.revision,
        "pubmed".into(),
        EvidenceSourceCoverage::searched("stroke".into(), 12),
    ).unwrap();
    let replaced = replace_project_direction(
        &root,
        searched.revision,
        "脑卒中居家上肢训练依从性".into(),
        vec!["居家训练".into(), "上肢".into(), "依从性".into()],
        vec!["中西医结合".into()],
        "用户明确取消旧方向".into(),
    ).unwrap();
    assert_eq!(replaced.evidence_sources["pubmed"].status, CoverageStatus::NotSearched);
    assert!(replaced.excluded_directions.iter().any(|item| item.direction == "中西医结合"));
}

#[test]
fn stale_revision_never_overwrites_current_context() {
    let root = workspace("cas");
    let initial = ensure_project_context(&root).unwrap();
    patch_project_scope(&root, initial.revision, vec!["上肢".into()], vec![]).unwrap();
    let error = patch_project_scope(&root, initial.revision, vec!["错误覆盖".into()], vec![]).unwrap_err();
    assert!(error.contains("revision"));
    assert!(!load_project_context(&root).unwrap().unwrap().active_scope.contains(&"错误覆盖".into()));
}
```

- [ ] **Step 5: Make all Project Context tests GREEN**

Run:

```powershell
cd rust
cargo test -p galen project_context::tests
```

Expected: all Project Context domain tests pass, including duplicate exclusion, invalid source key, corrupt JSON preservation, pending-path write failure, and render-only-current-state cases.

- [ ] **Step 6: Commit the domain store**

```powershell
git add rust/crates/galen/src-tauri/src/project_context.rs rust/crates/galen/src-tauri/src/lib.rs
git commit -m "feat(context): add host-authoritative project state"
```

---

### Task 2: Active-task deactivation and Tauri commands

**Files:**
- Modify: `rust/crates/galen/src-tauri/src/research_task.rs`
- Create: `rust/crates/galen/src-tauri/src/commands/project_context.rs`
- Modify: `rust/crates/galen/src-tauri/src/commands.rs`
- Modify: `rust/crates/galen/src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: Task 1 mutations and `commands::AppState`.
- Produces: `research_task::deactivate_active_task`, `get_project_context`, `replace_project_direction`, and `patch_project_scope` Tauri commands.

- [ ] **Step 1: Write failing active-task preservation test**

```rust
#[test]
fn deactivating_task_removes_pointer_but_preserves_snapshot() {
    let workspace = temp_workspace("deactivate");
    let task = create_task(&workspace, "旧课题".into(), "旧目标".into(), vec![]).unwrap();
    deactivate_active_task(&workspace).unwrap();
    assert!(load_active_task(&workspace).unwrap().is_none());
    assert_eq!(load_task(&workspace, &task.task_id).unwrap().task_id, task.task_id);
}
```

- [ ] **Step 2: Run RED and implement deactivation**

Run `cargo test -p galen research_task::tests::deactivating_task_removes_pointer_but_preserves_snapshot -- --exact` and confirm the missing-function failure. Implement:

```rust
pub fn deactivate_active_task(workspace: &Path) -> Result<(), String> {
    let _guard = lock_task_store()?;
    let pointer = galen_dir(workspace).join("active-task.json");
    if pointer.exists() {
        std::fs::remove_file(pointer).map_err(|error| format!("清除活动任务失败: {error}"))?;
    }
    Ok(())
}
```

- [ ] **Step 3: Write command serialization tests and implement commands**

Create thin Tauri commands that obtain `backend.get_workspace_root()`, reject a missing workspace, call Task 1 services, and return the updated context. After adding `deactivate_active_task`, update the domain-level `project_context::replace_project_direction` so it performs task deactivation itself after persisting the new context. Tauri commands and Agent tools must call this same domain function; neither may duplicate the lifecycle sequence.

- [ ] **Step 4: Register commands and run focused tests**

Add `pub mod project_context;` under `commands.rs`, register all three commands in `tauri::generate_handler!`, then add a replacement test proving both `ProjectContext.activeTaskId` and `.galen/active-task.json` are cleared while the old task snapshot remains readable. Run:

```powershell
cd rust
cargo test -p galen research_task::tests::deactivating_task_removes_pointer_but_preserves_snapshot
cargo check -p galen
```

- [ ] **Step 5: Commit task lifecycle and IPC**

```powershell
git add rust/crates/galen/src-tauri/src/research_task.rs rust/crates/galen/src-tauri/src/commands/project_context.rs rust/crates/galen/src-tauri/src/commands.rs rust/crates/galen/src-tauri/src/lib.rs
git commit -m "feat(context): expose project state mutations"
```

---

### Task 3: Model-facing Project Context tool

**Files:**
- Create: `rust/crates/galen/src-tauri/src/tools/project_context.rs`
- Modify: `rust/crates/galen/src-tauri/src/tools/mod.rs`
- Modify: `rust/crates/galen/src-tauri/src/capability.rs`

**Interfaces:**
- Consumes: Task 1 domain functions and `ToolContext.workspace_root`.
- Produces: `UpdateProjectContext` implementing `GalenTool`, registered as `update_project_context` in Research Pack.

- [ ] **Step 1: Add failing tool definition and permission tests**

```rust
#[test]
fn project_context_tool_is_a_write_tool() {
    let tool = UpdateProjectContext;
    assert_eq!(tool.definition().name, "update_project_context");
    assert!(tool.is_write());
}

#[tokio::test]
async fn replace_requires_reason_and_revision() {
    let (tool, ctx) = context("replace-validation");
    let error = tool.execute(json!({
        "operation": "replace_direction",
        "research_question": "新课题",
        "active_scope": ["上肢"]
    }), &ctx).await.unwrap_err();
    assert!(error.contains("expected_revision"));
}
```

- [ ] **Step 2: Run RED and implement the minimal tool**

Run `cargo test -p galen tools::project_context::tests` and confirm the missing module/type failure. Implement input parsing for exactly `replace_direction` and `patch_scope`, obtain the selected workspace from the mutex, call the Task 1 API, and serialize the complete updated context as JSON.

- [ ] **Step 3: Add failing mode and replacement behavior tests**

Assert Discuss mode returns the existing write-permission error through `ToolRegistry::execute_dynamic`, Auto mode persists the new question, and replacement clears the active task pointer through the domain operation used by both Tauri and tool paths.

- [ ] **Step 4: Register only in Research Pack and run capability tests**

Append `update_project_context` to `ResearchPack.tool_names`, register `tools::project_context::UpdateProjectContext`, and assert kernel-only registry still excludes it.

Run:

```powershell
cd rust
cargo test -p galen tools::project_context::tests
cargo test -p galen capability::tests
```

- [ ] **Step 5: Commit the Agent tool**

```powershell
git add rust/crates/galen/src-tauri/src/tools/project_context.rs rust/crates/galen/src-tauri/src/tools/mod.rs rust/crates/galen/src-tauri/src/capability.rs
git commit -m "feat(context): let agents update project state"
```

---

### Task 4: Dynamic context priority and evidence boundary

**Files:**
- Modify: `rust/crates/galen/src-tauri/src/context_engine.rs`
- Modify: `rust/crates/galen/src-tauri/src/context_engine_tests.rs`
- Modify: `rust/crates/galen/src-tauri/src/chat_loop.rs`

**Interfaces:**
- Consumes: `load_project_context` and `render_project_context`.
- Produces: a stable `project_context_summary` section inserted before task progress, plus execution guidance that requires available search tools for explicit literature requests.

- [ ] **Step 1: Write failing context projection tests**

```rust
#[test]
fn project_context_is_injected_before_plan_and_memory() {
    let ws = tmp_ws("project_context", &[]);
    seed_project_context(&ws, "脑卒中居家上肢训练依从性", &["居家训练", "上肢"]);
    let tail = build_turn_context("继续讨论这个课题", ChatMode::Auto, &Mutex::new(Some(ws)), false);
    assert!(tail.contains("当前项目状态（宿主权威）"));
    assert!(tail.find("当前项目状态").unwrap() < tail.find("科研计划进度").unwrap());
}

#[test]
fn incomplete_database_coverage_forbids_global_no_evidence_claims() {
    let ws = tmp_ws("coverage_guard", &[]);
    seed_pubmed_only_coverage(&ws);
    let tail = build_turn_context("总结证据", ChatMode::Auto, &Mutex::new(Some(ws)), false);
    assert!(tail.contains("不得声称整体没有证据"));
    assert!(tail.contains("CNKI=不可用"));
}
```

- [ ] **Step 2: Run RED and implement compact injection**

Run both named tests and verify missing-section failures. Add `project_context_summary` to `build_turn_context`, render only current state, and keep direct-answer contracts unchanged so simple questions remain fast.

- [ ] **Step 3: Add failing search-execution contract test**

Extend the existing explicit-search contract test so the generated policy contains “必须实际调用可用检索工具” and does not treat a Boolean query string as task completion.

- [ ] **Step 4: Implement search execution guidance and run context suite**

Update the search-specific dynamic policy without changing the cache-stable system prefix. Run:

```powershell
cd rust
cargo test -p galen context_engine_tests::context_tests
```

- [ ] **Step 5: Commit context assembly changes**

```powershell
git add rust/crates/galen/src-tauri/src/context_engine.rs rust/crates/galen/src-tauri/src/context_engine_tests.rs rust/crates/galen/src-tauri/src/chat_loop.rs
git commit -m "feat(context): inject authoritative project scope"
```

---

### Task 5: PubMed coverage lifecycle

**Files:**
- Modify: `rust/crates/galen/src-tauri/src/project_context.rs`
- Modify: `rust/crates/galen/src-tauri/src/tools/medical.rs`

**Interfaces:**
- Consumes: `ensure_project_context`, `update_evidence_coverage`, and the existing PubMed client.
- Produces: `begin_source_search`, `complete_source_search`, and `fail_source_search` helpers used by both PubMed search tools.

- [ ] **Step 1: Write failing durable lifecycle tests**

```rust
#[test]
fn source_search_lifecycle_is_revision_safe() {
    let root = workspace("search-lifecycle");
    let searching = begin_source_search(&root, "pubmed", "stroke adherence").unwrap();
    assert_eq!(searching.evidence_sources["pubmed"].status, CoverageStatus::Searching);
    let searched = complete_source_search(&root, "pubmed", 17).unwrap();
    assert_eq!(searched.evidence_sources["pubmed"].status, CoverageStatus::Searched);
    assert_eq!(searched.evidence_sources["pubmed"].result_count, Some(17));
}

#[test]
fn failed_search_is_not_counted_as_covered() {
    let root = workspace("search-failed");
    begin_source_search(&root, "pubmed", "stroke").unwrap();
    let failed = fail_source_search(&root, "pubmed", "network timeout").unwrap();
    assert_eq!(failed.evidence_sources["pubmed"].status, CoverageStatus::Failed);
}
```

- [ ] **Step 2: Run RED and implement lifecycle helpers**

Each helper reloads the latest persisted Revision immediately before its write. `complete_source_search` preserves the query summary written by `begin_source_search`; `fail_source_search` stores a sanitized reason capped at 240 characters.

- [ ] **Step 3: Integrate both PubMed search tools**

For `SearchPubMed` and `SearchRehabLiterature`:

```rust
let root = selected_workspace(ctx)?;
crate::project_context::begin_source_search(&root, "pubmed", query)?;
let papers = match ctx.medical.search_pubmed(query, limit).await {
    Ok(papers) => papers,
    Err(error) => {
        crate::project_context::fail_source_search(&root, "pubmed", &error.to_string())?;
        return Err(format!("PubMed search error: {error}"));
    }
};
crate::project_context::complete_source_search(&root, "pubmed", papers.len() as u64)?;
```

If no workspace is selected, reject the search before calling PubMed. A search cannot be reported as complete when its coverage state cannot be persisted.

- [ ] **Step 4: Run medical, context, and full Rust tests**

```powershell
cd rust
cargo test -p galen project_context::tests
cargo test -p galen tools::project_context::tests
cargo test -p galen
```

- [ ] **Step 5: Commit coverage persistence**

```powershell
git add rust/crates/galen/src-tauri/src/project_context.rs rust/crates/galen/src-tauri/src/tools/medical.rs
git commit -m "feat(evidence): track PubMed coverage"
```

---

### Task 6: Project Context and coverage UI

**Files:**
- Modify: `rust/crates/galen/src/types.ts`
- Create: `rust/crates/galen/src/hooks/useProjectContext.ts`
- Create: `rust/crates/galen/src/components/ProjectContextStrip.tsx`
- Create: `rust/crates/galen/src/components/EvidenceCoverageCard.tsx`
- Create: `rust/crates/galen/src/components/ProjectContext.test.tsx`
- Modify: `rust/crates/galen/src/App.tsx`
- Modify: `rust/crates/galen/src/styles/workbench.css`

**Interfaces:**
- Consumes: Task 2 Tauri commands.
- Produces: `useProjectContext(backendAvailable, workspaceRoot, messageCount)`, `ProjectContextStrip`, and `EvidenceCoverageCard`.

- [ ] **Step 1: Define frontend types and write failing component tests**

Add exact mirror types:

```ts
export type CoverageStatus = "not_searched" | "searching" | "searched" | "unavailable" | "failed";

export interface EvidenceSourceCoverage {
  status: CoverageStatus;
  searchedAt?: string | null;
  querySummary?: string | null;
  resultCount?: number | null;
  reason?: string | null;
}

export interface ProjectContext {
  schemaVersion: number;
  revision: number;
  projectId: string;
  researchQuestion: string;
  activeScope: string[];
  excludedDirections: Array<{ direction: string; reason: string; excludedAt: string }>;
  evidenceSources: Record<string, EvidenceSourceCoverage>;
  activeTaskId?: string | null;
  updatedAt: string;
}
```

Test that the strip renders the current question and scope, the coverage card maps all five statuses, and `null` context renders neither component.

- [ ] **Step 2: Run component tests and observe RED**

```powershell
cd rust/crates/galen
npm test -- src/components/ProjectContext.test.tsx
```

Expected: module resolution fails because the components do not exist.

- [ ] **Step 3: Implement the hook and components**

The hook calls `get_project_context` on workspace changes and after each completed message. It exposes:

```ts
{
  context,
  loading,
  error,
  refresh,
  replaceDirection(input),
  patchScope(input)
}
```

Mutation methods always send `context.revision`; on a Revision error they refresh and throw a user-facing “项目状态已更新，请基于最新状态重试”。

- [ ] **Step 4: Wire UI without adding canonical React state**

Instantiate the hook in `App.tsx`. Render `ProjectContextStrip` below `AppTopBar` whenever context exists. Render `EvidenceCoverageCard` in the execution view’s context area and the daily workbench summary area. Keep editing collapsed by default and do not expose excluded history unless the user opens the editor.

- [ ] **Step 5: Add styles and run all frontend tests/build**

Use only existing variables from `tokens.css`. Run:

```powershell
cd rust/crates/galen
npm test
npm run build
```

Expected: all prior 34 tests plus new Project Context tests pass; TypeScript and Vite build succeed.

- [ ] **Step 6: Commit the UI**

```powershell
git add rust/crates/galen/src/types.ts rust/crates/galen/src/hooks/useProjectContext.ts rust/crates/galen/src/components/ProjectContextStrip.tsx rust/crates/galen/src/components/EvidenceCoverageCard.tsx rust/crates/galen/src/components/ProjectContext.test.tsx rust/crates/galen/src/App.tsx rust/crates/galen/src/styles/workbench.css
git commit -m "feat(context): show project scope and evidence coverage"
```

---

### Task 7: Regression cases and release evidence

**Files:**
- Create: `evals/cases/e13_same_topic_continuity.toml`
- Create: `evals/cases/e14_direction_replacement.toml`
- Create: `evals/cases/e15_search_execution.toml`
- Create: `evals/cases/e16_coverage_boundary.toml`
- Create: `evals/cases/e17_project_context_recovery.toml`
- Create: `evals/cases/e18_project_context_isolation.toml`
- Modify: `rust/crates/galen/src-tauri/src/eval.rs`
- Modify: `rust/crates/galen/src-tauri/src/bin/eval.rs`
- Modify: `evals/README.md`

**Interfaces:**
- Consumes: persisted Project Context and existing immutable `RunRecord` assertions.
- Produces: Project Context assertion fields and E13–E18 validation/run contracts.

- [ ] **Step 1: Write failing evaluator assertion tests**

Extend the case requirement schema with:

```rust
#[serde(default)]
pub project_question: Option<String>,
#[serde(default)]
pub active_scope: Vec<String>,
#[serde(default)]
pub excluded_directions: Vec<String>,
#[serde(default)]
pub source_status: BTreeMap<String, CoverageStatus>,
```

Add tests proving a stale question, an excluded direction in active scope, a missing PubMed call, and a global no-evidence phrase under partial coverage each fail a hard gate.

- [ ] **Step 2: Run RED and implement evaluator loading**

Run `cargo test -p galen eval::tests::project_context_gate` and confirm missing-field failure. Load `project-context.json` from the run workspace after the Agent completes and add named assertions to `RunRecord` without injecting answers into the prompt.

- [ ] **Step 3: Add the six case contracts**

Use isolated fixtures and declare exact hard gates:

- E13 preserves the same question and scope after follow-up turns;
- E14 replaces the question, excludes the old direction, and clears the old active task;
- E15 requires `search_pubmed` or `search_rehab_literature` in the tool trace;
- E16 forbids `没有证据|不存在研究|无相关文献` while CNKI/Wanfang remain uncovered;
- E17 seeds a persisted context and verifies a new process loads it unchanged;
- E18 seeds two workspace contexts and verifies neither question appears in the other output or state file.

- [ ] **Step 4: Validate all contracts without a model call**

```powershell
cd rust
cargo run -p galen --bin eval -- validate
```

Expected: every existing and new TOML case parses and passes fixture validation.

- [ ] **Step 5: Run deterministic and smoke gates**

```powershell
cd rust
cargo test -p galen
cargo run -p galen --bin eval -- run --case E14 --model deepseek-v4-flash --repeat 1 --output ../evals/runs/e14-project-context-smoke.jsonl
cargo run -p galen --bin eval -- run --case E16 --model deepseek-v4-flash --repeat 1 --output ../evals/runs/e16-coverage-smoke.jsonl
```

Expected: both smoke runs pass every hard gate. Keep JSONL under ignored `evals/runs/`; do not commit model output.

- [ ] **Step 6: Update evaluation documentation and commit**

Document the new cases, state that smoke is not a Release baseline, and include the K=5/K=20 progression.

```powershell
git add evals/cases/e13_same_topic_continuity.toml evals/cases/e14_direction_replacement.toml evals/cases/e15_search_execution.toml evals/cases/e16_coverage_boundary.toml evals/cases/e17_project_context_recovery.toml evals/cases/e18_project_context_isolation.toml rust/crates/galen/src-tauri/src/eval.rs rust/crates/galen/src-tauri/src/bin/eval.rs evals/README.md
git commit -m "test(context): gate project continuity and coverage"
```

---

### Task 8: Final verification and handoff

**Files:**
- Modify: `docs/GALEN_AI_HANDOFF_2026-08-29.md`

**Interfaces:**
- Consumes: all prior tasks and their commits.
- Produces: verified implementation status and exact remaining limitations.

- [ ] **Step 1: Run formatting and diff checks**

```powershell
git diff --check
cd rust
cargo fmt --all --check
```

- [ ] **Step 2: Run full relevant verification**

```powershell
cd rust
cargo check --workspace
cargo test -p galen
cd crates/galen
npm test
npm run build
```

- [ ] **Step 3: Inspect the working tree and commit only handoff changes**

Update the handoff with the Project Context path, commands, test counts, smoke evidence, and the explicit limitation that Chinese database connectors remain unavailable.

```powershell
git add docs/GALEN_AI_HANDOFF_2026-08-29.md
git commit -m "docs: hand off project context architecture"
```

- [ ] **Step 4: Report the integration boundary**

Report the isolated worktree branch, commits, verification output, any failed smoke caused by external model availability, and the untouched dirty files in the primary checkout. Do not merge, push, tag, or publish without a separate user request.
