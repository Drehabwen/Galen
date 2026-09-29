# Task Plan

## Active Task: Galen -> RehabMain Research Feedback Loop

### Objective
- Return machine-readable observation decisions and follow-up actions from Galen to RehabMain so the first connector supports iterative research execution, not only one-way ingestion.

### Constraints
- Link feedback by explicit upstream assessment ID and RehabID.
- Preserve accept, exclude, correction, recapture, and retest as analyzable research events.
- Keep existing case files backward compatible and make feedback publishing idempotent.

### Steps
- [completed] Promote upstream assessment provenance from observation notes into a typed field and add research follow-up actions.
- [completed] Publish idempotent connector feedback after an observation review.
- [completed] Read and display actionable feedback in RehabMain.
- [completed] Run focused Rust, frontend, desktop, and TypeScript verification.

### Verification
- Galen rehab-context and connector Rust tests.
- Galen RehabContextPanel frontend test and TypeScript check.
- RehabMain connector writer/reader tests and TypeScript check.

### Outcome
- Galen observations now retain a typed upstream assessment ID instead of relying on prose notes for provenance.
- Observation reviews carry `none`, `recapture`, or `schedule_retest` and publish idempotent feedback items to the RehabMain bridge.
- RehabMain presents accept, exclude, correction, recapture, and retest events as a live research execution queue.
- Verification passed: Galen connector 11/11, timeline 8/8, RehabContextPanel 2/2, RehabMain desktop connector 3/3, and both TypeScript checks.
- Remaining loop: RehabMain must eventually acknowledge task completion and point Galen to the resulting new assessment.

## Active Task: Real-World Rehabilitation Data Loop

### Objective
- Make Galen's first real-world rehabilitation data path trustworthy from local connector discovery through pseudonymous identity, provenance-preserving timeline import, review, and research use.

### Constraints
- Keep raw identity and source files local; the research timeline must use stable pseudonymous IDs and immutable source references.
- Treat connector observations as candidates until quality or human review verifies them.
- Preserve screening boundaries: do not import or present estimated diagnosis/Cobb-angle fields as verified clinical observations.
- Preserve existing user changes; remove the rejected, unvalidated benchmark draft created during the interrupted direction.

### Steps
- [completed] Map the current connector -> normalized export -> RehabID timeline -> research context path and identify its P0 trust gaps.
- [completed] Remove name-derived identifiers, retain source-record provenance, and default unreviewed connector observations to candidate.
- [completed] Block diagnosis-like estimated Cobb fields and expose candidate/review counts in the import receipt and UI.
- [completed] Connect the governed RehabID timeline to the Agent's read-only rehab tool with verified-by-default access.
- [completed] Run focused connector/timeline/tool/frontend tests and document the remaining protocol-registry and outcome-feedback gaps.

### Verification
- Run focused Rust connector and governed timeline tests plus affected frontend tests.
- Confirm a connector fixture without `shortCode` never persists or displays the supplied name.
- Confirm connector observations remain candidate and estimated Cobb fields are excluded.
- Run `git diff --check` without modifying unrelated worktree changes.

### Outcome
- The first local real-world path now runs from Connector discovery and confirmation through pseudonymous RehabID persistence into Agent-readable, provenance-bearing observations.
- Connector observations remain candidates until reviewed; Agent access excludes them by default and marks them explicitly when requested for quality audit.
- Estimated Cobb output is blocked at extraction, while observed ATR/scoliometer records remain reviewable.
- Architecture and next gaps are recorded in `docs/real-world-rehabilitation-data-loop.md`.

## Active Follow-up: Protocol Registry And Observation Review

### Objective
- Add a versioned rehabilitation measurement protocol registry and a generic, auditable review path that can promote, reject, or correct candidate observations without silent mutation.

### Constraints
- Keep screening evidence distinct from diagnosis and model-estimated clinical truth.
- Unknown or unit-mismatched connector fields remain candidates; excluded fields cannot be promoted.
- Every human decision records reviewer, reason, previous state, resulting state, timestamp, and case revision.
- Preserve existing conflict-review behavior and backward-compatible case deserialization.

### Steps
- [completed] Define and validate the versioned protocol registry with canonical metrics, aliases, units, evidence kinds, and allowed uses.
- [completed] Attach protocol resolution to governed imports and block excluded measurements centrally.
- [completed] Add revision-safe accept/reject/correct observation review commands and audit history.
- [completed] Add UI review actions and run focused backend/frontend verification.

### Verification
- Test alias resolution, unit mismatch downgrade, excluded metric rejection, and unknown metric fallback.
- Test accept/reject/correct decisions, stale revision rejection, audit persistence, and cohort recomputation.
- Run affected frontend tests, TypeScript checks, and `git diff --check`.

### Outcome
- Added `rehab-screening-core@1.0.0` as the runtime authority for canonical names, units, evidence kinds, screening/research use, and excluded fields.
- Governed imports now normalize aliases, downgrade unregistered or unit-mismatched observations, and centrally reject diagnosis-like Cobb estimates.
- Candidate observations can be accepted, rejected, or corrected from the RehabID UI; every decision stores before/after state, reviewer, reason, timestamp, and revision-safe audit history.
- Existing bundles migrate their pending-review count on first load, and Agent access remains verified-only by default.
- Verification passed: protocol 4/4, timeline/review 8/8, connector 10/10, rehab tool 2/2, frontend 6/6, TypeScript, and `git diff --check`.

## Objective
- Remove high-risk production hardcoding and reduce Galen technical debt without changing research behavior or test fixtures.
- Unify Galen's existing evaluation assets into a continuous-improvement gate with deterministic context-protocol coverage.

## Constraints
- Preserve existing dirty-worktree changes and generated release assets.
- Do not rewrite deterministic eval/demo fixtures merely to eliminate literals.
- Prefer one authoritative configuration source and explicit empty states over hidden sample defaults.
- Rust verification is limited until `cargo` is available in the current shell.

## Steps
- [completed] Inventory production hardcoding and select a bounded first cleanup batch.
- [completed] Remove duplicated frontend model presets and defer empty-config defaults to the backend authority.
- [completed] Remove production UI defaults tied to specific AIS/ATH sample cases.
- [completed] Add regression tests for configuration-driven behavior and empty states.
- [completed] Run frontend tests/build and record remaining debt by priority.
- [completed] Centralize backend provider defaults and harden model configuration serialization.
- [completed] Run targeted Rust verification using the installed Cargo executable; `cargo check -p galen` passes.
- [completed] Extract connector discovery/import state from `App.tsx` into a dedicated hook.
- [completed] Prevent stale artifact and benchmark requests from overwriting state after workspace changes.
- [completed] Replace silent workspace/model initialization failures with visible runtime errors.
- [completed] Unify model-required message dispatch so thread actions cannot send an empty model alias.
- [completed] Surface partial Session persistence and listener-registration failures instead of logging them silently.
- [completed] Make mode loading/switching atomic, race-safe, and user-visible on failure.
- [completed] Split `ResearchExecutionThread.tsx` into focused status, messages, connector, and composer components.
- [completed] Lock the extracted thread component boundaries with focused interaction tests.
- [completed] Split document preview renderers from the canvas shell and sanitize converted DOCX HTML.
- [completed] Extract Session chat restoration, event lifecycle, sending, and auto-run orchestration into a hook.
- [completed] Lazy-load PDF, code-highlighting, DOCX, and XLSX preview dependencies by artifact format.
- [completed] Add a minimal execution context with explicit discuss/inspect/execute/verify modes and correction retention.
- [completed] Inject the execution context into backend turns without polluting durable user messages.
- [completed] Enforce mode/tool boundaries, no-build constraints, and real task-specific tool budgets.
- [completed] Add behavior regression tests derived from recent failure cases.
- [completed] Move ExecutionContext ownership to the backend and persist it at workspace/topic scope for main and Session conversations.
- [completed] Route main chat, research orchestration messages, and Session messages through one frontend send gateway.
- [completed] Compile every turn's task contract from the backend-authoritative context rather than optional frontend envelopes.
- [completed] Add regression coverage for correction retention, inherited internal messages, explicit constraint release, and gateway policy.
- [completed] Move reviewer identity defaults to the backend and centralize rehab workspace validation.
- [completed] Add a versioned evaluation suite manifest that maps code changes to deterministic, model, RAG, medical, UI, and architecture lanes.
- [completed] Add a non-model suite validator/runner and machine-readable run summary without replacing the Rust evaluator.
- [completed] Expand backend execution-context protocol regression coverage for update, inherit, release, continuation, and workspace isolation.
- [completed] Run only the new deterministic validation and record remaining live-model/release gates.
- [completed] Define executable PCOLLAB, PSEC, and PJUDGE benchmark contracts with private/public separation and explicit promotion status.
- [completed] Add deterministic validators and scorers for collaboration efficiency, adversarial safety, and judge calibration.
- [completed] Register the new dimensions and benchmark lanes in the unified suite without weakening existing hard gates.
- [completed] Run contract/unit validation and record which expert/live-model gates remain intentionally incomplete.
- [completed] Remove unconditional MCP startup from tool-free direct answers based on the 36.55 s setup trace.
- [completed] Add per-case TTFR/total-latency hard gates so a correct but unusably slow answer cannot pass E01.
- [completed] Rebuild the evaluator once and repeat the same E01 smoke for before/after evidence.
- [completed] Add a ScienceAgentBench-style rehabilitation coding pilot with a self-contained Python deliverable and hidden executable tests.
- [completed] Run the identical pilot through Galen and the locally available Codex agent, preserving workspaces and raw records.
- [completed] Score both agents on execution correctness, generalization, safety, latency, and artifact integrity; document comparability limits against official ScienceAgentBench.
- [completed] Locate and verify local Codex CLI and Claude Code runtimes without relying on stale repository documentation.
- [completed] Restore a version-pinned isolated Claude Code runtime if no executable remains, without changing user-global agent configuration.
- [completed] Prepare the official public ScienceAgentBench assets and document the exact runnable subset versus access-controlled artifacts.
- [completed] Configure a benchmark-owned same-model DeepSeek V4.1 Flash path for Codex and Claude Code using runtime-only secret injection, without mutating the user's global Galen default.
- [completed] Run environment/adapter checks before any formal public benchmark and record reproducibility limits.
- [completed] Add a deterministic ScienceAgentBench preflight with separate native-agent and controlled-core comparison tracks; discovered skills are capabilities in the native track, not automatic contamination.
- [completed] Obtain, validate, and safely extract the access-controlled official `benchmark_verified.zip` from the authenticated OSU SharePoint session.
- [completed] Build and run an idempotent official ClinTox single-instance pilot for the staged Codex and Claude predictions.
- [completed] Record isolated execution logs, validity, ROC-AUC threshold result, and remaining full-suite requirements.
- [completed] Calibrate the ClinTox execution environment by requiring the untouched official gold program to pass before agent scoring.
- [completed] Select and validate a CPU-only task image derived from the shared official Docker base, omitting the unnecessary optional DGL/CUDA task dependency.
- [completed] Rerun untouched Codex and Claude predictions only after gold calibration succeeds.
- [pending] Run Codex in a clean Windows account/container because Codex 0.157.0 still scans `%USERPROFILE%/.agents/skills` despite `skip_host_skill_discovery`.

## Verification
- `npx vitest run --reporter=dot` in `rust/crates/galen`.
- `npm run build` in `rust/crates/galen`.
- `cargo check -p galen` in `rust` when Cargo is available.
- Re-scan production sources for model IDs, absolute user paths, and fixed sample case IDs.
- Run focused frontend execution-context tests and backend context/task-contract tests without a full release build.

## ScienceAgentBench Contract Baseline

### Objective
- Make benchmark runtime paths explicit and machine-verifiable before generation, then establish a reproducible ClinTox baseline that separates contract compliance from scientific performance.

### Constraints
- Preserve the original failed Codex and Claude programs as immutable diagnostic evidence.
- Use the verified benchmark, the calibrated shared CPU environment, and isolated outputs; do not start the 102-task suite.
- Treat program validity and artifact production as gates before ROC-AUC or similarity scores.

### Steps
- [completed] Inspect the generation prompt, runtime mount layout, and current runner to define one authoritative task contract.
- [completed] Implement the contract manifest, prompt injection, and fail-fast preflight with focused tests.
- [completed] Add a clearly labeled contract-aware ECFP + logistic-regression baseline without overwriting the original agent outputs.
- [completed] Run the official ClinTox evaluator and record validity, threshold result, provenance, and residual limitations.

### Verification
- Run focused Python tests for contract rendering and path validation.
- Run the baseline in the calibrated CPU image and retain isolated raw logs.
- Validate JSON artifacts and run `git diff --check`.

## Contract-Aware Agent Competition

### Objective
- Compare Galen, Codex, and Claude on the same ClinTox generation task using DeepSeek V4 Pro and the pinned contract baseline.

### Constraints
- Use one prompt, model ID, task contract, CPU image, evaluator, timeout, and output schema.
- Keep native orchestration differences visible; do not repair generated programs after submission.
- Preserve each raw generation trace, generated program, execution log, and provenance hash.

### Steps
- [completed] Extend the generation adapter and candidate scorer for a model-locked, isolated three-agent run.
- [completed] Generate one untouched candidate from Codex, Claude, and Galen.
- [completed] Execute all candidates against the official evaluator and compare them with the 0.8398 baseline.
- [completed] Record the result table, limitations, and next repeat-run requirements.

## Galen P0 Flash Remediation

### Objective
- Correct Galen's generation/runtime reasoning and dependency validation, then rerun the same ClinTox task with all agents locked to DeepSeek V4.1 Flash (`deepseek-flash`).

### Constraints
- Do not mutate the user's global model configuration or persist API credentials in the repository or run artifacts.
- Preserve the V4 Pro first-round results as historical evidence; Flash results use a new run directory.
- Generated candidate programs remain untouched after submission.

### Steps
- [completed] Add phase-aware runtime context, verified capability symbols, tensor semantics, and model-identity gates.
- [completed] Fix Galen's Python artifact preview hard gate and add focused regression tests.
- [completed] Create an ephemeral Galen Flash route and verify all three adapters resolve `deepseek-flash`.
- [completed] Run the same one-shot generation and official ClinTox evaluation under Flash.
- [completed] Record the before/after result and remaining repeat-run requirements.

## Galen Invalid-Molecule Remediation

### Objective
- Make malformed-molecule handling an explicit, testable generation contract and reject non-compliant candidates before expensive model training.

### Constraints
- Preserve contract v1/v2 and every previously generated candidate/result as immutable evidence.
- Do not hand-edit generated programs; rerun only Galen with the locked `deepseek-flash` model.
- Preserve every test row and order, use fixed-width deterministic features, and prohibit label-dependent fallback behavior.

### Steps
- [completed] Define contract v3 and a candidate preflight for fixed-width deterministic invalid-molecule handling.
- [completed] Add focused contract/preflight regression tests and validate runner provenance.
- [completed] Generate one untouched Galen candidate in a new isolated run and verify the recorded model identity.
- [completed] Run the official ClinTox scorer only if preflight passes, then report validity, ROC-AUC, and baseline delta.

## Galen Generation Tool Reliability

### Objective
- Remove avoidable generation-phase tool calls and make malformed streamed tool arguments recoverable instead of misreporting them as missing fields.

### Constraints
- Keep ordinary coding and data-analysis tool access unchanged.
- Do not repair generated benchmark programs inside the runtime.
- Preserve the repeated-failure breaker as a last resort rather than using it as normal control flow.

### Steps
- [in_progress] Add a bounded generation-only tool contract with chunked artifact delivery guidance.
- [pending] Replace silent tool-argument JSON fallback with explicit parse diagnostics and one parsed representation.
- [pending] Add focused contract/parser regression tests and rebuild the release evaluator.
- [pending] Rerun one Galen Flash generation and compare tool errors without starting repeated benchmark scoring.

## Constraint-Aware Literature Retrieval

### Objective
- Prevent topically similar but user-mismatched PubMed records from entering Galen's evidence context.

### Constraints
- Preserve the existing PubMed provider, provenance ledger, citation verification, and backward-compatible simple queries.
- Prefer explicit rejection with mismatch reasons over presenting low-fit papers as usable evidence.
- Implement deterministic first-stage gating now; defer model training, cross-encoder hosting, and reinforcement learning.

### Steps
- [completed] Audit query construction, provider adapters, raw result fidelity, ranking, compaction, and final context injection end to end.
- [completed] Add structured required/excluded scientific concepts and publication-type constraints where the audit showed they are needed.
- [completed] Gate and classify candidates as matched, ambiguous, or rejected with explicit mismatch reasons and rejection summaries.
- [completed] Add focused regression tests for population mismatch, exclusion handling, Chinese-query fallback, and legacy compatibility.
- [completed] Run targeted Rust verification and document the remaining semantic-reranker gap.

### Verification
- Run focused `tools::medical` and tool-schema tests.
- Run `cargo check -p galen` if targeted tests pass.
- Run `git diff --check` and preserve unrelated working-tree changes.

### Audit findings and bounded outcome
- The decisive defect was not PubMed relevance ordering alone: Galen used unconstrained lexical coverage as if it were semantic relevance, accepted low-fit records, and let the model over-constrain retries until PubMed returned zero results.
- Chinese research questions collapsed into ineffective ranking tokens; the new ranking anchor falls back to the English PubMed query when no usable English terms exist.
- PubMed XML parsing retained only the first structured abstract section and direct title text; the parser now preserves all labeled abstract sections and inline title markup.
- `search_pubmed` now separates broad retrieval from deterministic screening through required concepts, exclusions, and publication types. Rejected records do not enter the model-visible paper list; ambiguous records are explicitly non-citable until fetched and checked.
- Search reports now expose short abstract evidence, screening rationale, and aggregate rejection reasons, while the context budget preserves complete top records.
- The remaining gap is cross-provider normalization and semantic reranking for MCP sources (Crossref, Semantic Scholar, CNKI); this batch deliberately leaves that larger adapter refactor for a separate change.

## Unified MCP Literature Screening

### Objective
- Normalize recognized Crossref, Semantic Scholar, and CNKI search responses before they enter model context, then apply provider-independent eligibility screening and explicit degradation rules.

### Constraints
- Preserve raw MCP responses for provenance hashing and provider-specific status detection.
- Never convert provider failure, partial output, or an unparseable payload into a successful zero-result search.
- Do not guess arbitrary MCP tools or silently treat unknown schemas as screened evidence.
- Keep provider adapters deterministic and dependency-free in this batch.

### Steps
- [in_progress] Inventory observed provider payload envelopes and define one normalized literature candidate schema.
- [pending] Implement declared-path extraction plus Crossref, Semantic Scholar, and CNKI field adapters.
- [pending] Apply common lexical/constraint screening and return a bounded evidence-oriented report to the model.
- [pending] Preserve raw-output provenance while recording normalized result counts and safe degradation states.
- [pending] Add fixture-driven adapter, rejection, partial-response, and execution-path regression tests.
- [pending] Run targeted Rust checks and record unsupported schema/residual semantic-reranker risks.

### Verification
- Run focused `tools::research` adapter tests and MCP dispatch tests.
- Run `cargo check -p galen` without a full workspace build.
- Run `git diff --check` and preserve unrelated changes.

## Outcome
- Removed duplicated model IDs from the production onboarding/settings UI; configured models now come from the backend, while an empty configuration defers to the backend default template.
- Removed implicit `AIS-C025` and dataset-path state from the production Rehab ID panel; imports now require explicit inputs.
- Replaced the fixed `ATH-001` connector instruction with a case-ID placeholder.
- Added focused tests for model-driven onboarding and explicit rehab import state.
- Verified 17 frontend test files / 57 tests pass and the production frontend build succeeds.
- Second pass centralized backend provider defaults, removed string-injected TOML, validated model aliases, extracted connector orchestration, fixed stale async updates, centralized rehab workspace checks, and surfaced previously silent operational failures.
- `App.tsx` dropped from roughly 544 to 476 lines after connector orchestration moved to `useConnectorImport`.
- `ResearchExecutionThread.tsx` dropped from roughly 738 to 141 lines and now composes dedicated status, message, connector, and composer components.
- `ResearchDocumentCanvas.tsx` dropped from 378 to 62 lines; format dispatch, text previews, and binary previews are isolated, and converted DOCX HTML is sanitized before rendering.
- `SessionChat.tsx` dropped from 316 to 125 lines; restoration, event lifecycle, sending, and auto-run orchestration now live in `useSessionChat`, which also resets state between nodes, displays streaming output, and cleans up partial listener registration.
- Current verification: `npx tsc --noEmit` passes; 19 frontend test files / 62 tests pass; `git diff --check` passes.
- Frontend production build passes. The document canvas entry chunk dropped from 1,557.50 kB to 5.00 kB; the on-demand code highlighter dropped from 642.71 kB to 95.99 kB after switching to registered Prism languages, and no business JS chunk exceeds 500 kB.
- Rust `cargo check -p galen` passes using `C:\Users\DORAT\.cargo\bin\cargo.exe`; the subsequent debug link build was stopped when work moved to lazy loading.
- Next priorities: split `App.tsx` and `WelcomeWizard.tsx`; remove remaining silent errors in conversation/research-task restoration; add CI frontend gates.
- Added a per-turn execution context that retains user corrections across “继续”, keeps raw user messages authoritative in durable history, and sends a bounded context envelope only to the model.
- Added hard discuss/inspect/execute/verify tool boundaries, targeted verification semantics, explicit no-build enforcement, and restored each task contract's real tool-turn budget instead of raising every task to 36 turns.
- Execution-context verification: `npx tsc --noEmit` passes; 4 focused Vitest cases pass; `git diff --check` passes. A targeted Rust test was stopped after 60 seconds with no result, and its lingering compiler processes were terminated; Rustfmt is currently unavailable because the installed executable exits with Windows status `0xC0000135`.
- Unified-context outcome: `.galen/execution-context.json` is now the backend authority and is archived with a new research topic. Direct user messages use the explicit `update` policy; orchestration and Session auto-run messages use `inherit`. All production frontend sends pass through `agentGateway.ts`, and the obsolete React-owned context implementation was removed.
- Unified-context verification: `npx tsc --noEmit` passes; `src/agentGateway.test.ts` passes 2/2; no production code directly invokes `send_message`; `git diff --check` passes. The focused Rust test command again exceeded 60 seconds without output and was stopped, with no Cargo/Rust processes left running.
- Added `continuous-improvement-v1`, a unified 9-lane / 8-dimension evaluation manifest spanning PR, nightly, and release gates while retaining the Rust evaluator as the scoring authority.
- Added a standard-library-only suite validator/planner/runner. It validates paths and contracts without compiling or calling a model, selects lanes from current Git changes, writes non-overwriting JSON reports for explicit runs, and treats delegated lanes as incomplete rather than falsely passed.
- Expanded deterministic execution-context coverage from two to six tests: update versus inherit ownership, continuation retention, explicit build/generation release, and workspace isolation.
- Evaluation-foundation verification: manifest validation and change-aware PR planning pass; 6 Python orchestration tests pass; the focused frontend gateway suite passes 2/2; package-level `npm run eval:validate` and `npm run eval:plan` pass; `git diff --check` passes. No prebuilt native evaluator was present, so native case/RAG validation and the new Rust tests remain explicit PR gates rather than claimed results.
- Expanded the unified suite from 9 lanes / 8 dimensions to 13 lanes / 12 dimensions by adding human collaboration, evaluator validity, security/privacy, and multimodal/temporal coverage.
- Added 17 executable benchmark contracts: PCOLLAB 6, PSEC 5, and PJUDGE 6. Event-based scorers require both safety and utility; judge scoring restores semantic candidate identity across mirrored A/B order and blocks promotion while expert review is pending.
- Added a framework-neutral scoring CLI that accepts immutable observation records and refuses to overwrite reports, so Galen, Inspect, and external agents can be compared against the same gold contracts.
- Extended-benchmark verification: all 17 contracts validate; 13 Python tests pass; unified suite validation reports 13 lanes / 12 dimensions; `npm run eval:benchmarks` and change-aware `npm run eval:plan` pass. Live-model PCOLLAB/PSEC execution remains a declared nightly gate, and PJUDGE remains deliberately non-promotable until rehabilitation experts review the clinical collaboration pair.
- First execution smoke: the prebuilt release evaluator validates all 37 native CaseSpecs and the 7-query AIS RAG dataset; E01 completed 1/1 with all native hard gates, quality 1.000, 0 tool calls/errors, 1,423 input and 77 output tokens. TTFR was 37.1 s and total latency 38.5 s, so this run passes native correctness but would fail the existing 5 s collaboration responsiveness target; one run is not a promotable baseline. The optional CNKI MCP executable was unavailable but did not affect this no-tool case.
- Added Galen-SciCode Pilot v1 as the 14th continuous-improvement lane and a 13th `scientific_reproducibility` dimension. Its first valid product-level comparison scored Galen + deepseek at 77.5/100 in 65.997 s and Codex CLI 0.146.0 at 100/100 in 262.543 s. Galen failed one hidden quality-accounting case, exceeded model/tool budgets, and exposed a command-workspace cwd defect. These are single-run diagnostic results, not an official ScienceAgentBench score or stable ranking.
- ScienceAgentBench ClinTox CPU pilot: the untouched official gold program passed (`valid_program=1`, `success_rate=1`, data/function correctness true, official ROC-AUC threshold 0.77 passed). Untouched Codex and Claude programs both failed before model execution (`valid_program=0`, `success_rate=0`) because their dataset discovery omitted the official `/testbed/benchmark/datasets/clintox` mount. Their CodeBERT scores (0.8021 and 0.7802) therefore do not indicate executable correctness. All three used the same calibrated CPU task environment with isolated images and outputs; optional DGL/CUDA task dependencies were not installed.
- Added the versioned `scienceagentbench-clintox-v1` runtime contract and injected its authoritative paths into generation prompts. Main preflight now exposes a dedicated `clintox_pilot_ready` gate. The leakage-free ECFP + class-balanced logistic-regression baseline passes the official evaluator with ROC-AUC `0.8398049301242236`, above the `0.77` threshold; its program, contract, train/test data, and image hashes are pinned. Three anomalous training SMILES are handled deterministically (two relaxed sanitizations and one zero-vector fallback), while all 292 test rows use standard fingerprints. Original Codex/Claude failures remain immutable and are not relabeled as baseline results.
- First contract-aware DeepSeek V4 Pro competition: identical prompt hash, contract, CPU image, data, and official evaluator were used for one untouched program from each framework. Claude passed (`valid_program=1`, ROC-AUC `0.7867818323`, official success 1) but remained below the deterministic 0.8398 baseline. Codex produced a valid ordered artifact but confused task and class axes, scoring `0.2283643892` and failing the threshold. Galen honored runtime paths but imported an unavailable PyTorch namespace symbol, so it exited before producing predictions. This is a defect-discovery run, not a stable ranking; Codex also remains native-track-only because host skill discovery was observed.
- DeepSeek V4.1 Flash P0 remediation: model lock now requires API model ID `deepseek-flash` for Galen, Codex, and Claude; Galen uses an ephemeral benchmark-only route without changing global configuration. Contract v2 separates generation and runtime paths, pins valid DeepChem symbols, and defines positive-class tensor semantics. Galen's internal delivery improved from quality 0.909/fail to 1.000/pass and no longer made the prior import or tensor-axis mistakes. In official execution, Codex passed at ROC-AUC `0.8169707557`; Claude and Galen both failed on ragged ECFP arrays produced by three malformed training SMILES. The next P0 is a general invalid-molecule policy; five-repeat reliability testing remains premature.
- Invalid-molecule P0 remediation: contract v3 and the fail-fast candidate preflight require fixed 1024-wide float32 ECFP vectors, deterministic zero fallback, label independence, fallback observability, and exact row preservation. The untouched Galen + `deepseek-flash` candidate passed all 10 preflight checks and the official CPU evaluator (`valid_program=1`, ROC-AUC `0.7948806289`, threshold pass, data/function correctness true). It preserved all 292 test rows and reported train fallback `3/1192`, test fallback `0/292`. This converts the prior execution failure into a valid result, though it remains 0.044924 below the deterministic baseline and needs repeated-seed reliability testing before any ranking claim.
- E01 latency fix: the trace showed 36,550 ms in unconditional MCP startup before a no-tool direct answer. Bounded built-in-only contracts now skip MCP initialization; literature and generic open-ended tasks retain it. E01 now enforces TTFR <=5,000 ms and total <=15,000 ms as hard gates. After one release rebuild, the identical live smoke reduced MCP setup 36,550→0 ms, TTFR 37,117→807 ms (-97.8%), and total 38,479→1,929 ms (-95.0%); both new latency gates passed. The rebuilt release target completed with one pre-existing dead-code warning and no errors; `git diff --check` passes.
