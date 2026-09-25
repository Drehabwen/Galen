# Task Plan

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
- [in_progress] Build and run an idempotent official ClinTox single-instance pilot for the staged Codex and Claude predictions.
- [pending] Record isolated execution logs, validity, ROC-AUC threshold result, and remaining full-suite requirements.
- [pending] Run Codex in a clean Windows account/container because Codex 0.157.0 still scans `%USERPROFILE%/.agents/skills` despite `skip_host_skill_discovery`.

## Verification
- `npx vitest run --reporter=dot` in `rust/crates/galen`.
- `npm run build` in `rust/crates/galen`.
- `cargo check -p galen` in `rust` when Cargo is available.
- Re-scan production sources for model IDs, absolute user paths, and fixed sample case IDs.
- Run focused frontend execution-context tests and backend context/task-contract tests without a full release build.

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
- E01 latency fix: the trace showed 36,550 ms in unconditional MCP startup before a no-tool direct answer. Bounded built-in-only contracts now skip MCP initialization; literature and generic open-ended tasks retain it. E01 now enforces TTFR <=5,000 ms and total <=15,000 ms as hard gates. After one release rebuild, the identical live smoke reduced MCP setup 36,550→0 ms, TTFR 37,117→807 ms (-97.8%), and total 38,479→1,929 ms (-95.0%); both new latency gates passed. The rebuilt release target completed with one pre-existing dead-code warning and no errors; `git diff --check` passes.
