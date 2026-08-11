# Galen Reuse and Consistency Audit

## Objective

Audit the current Galen codebase for reuse, naming, and architectural consistency before the next consolidation pass.

## Summary

Galen is converging in product direction, but the code still carries three overlapping identities:

- `Galen`: the current doctor/research workstation product.
- `Claw`: the inherited CLI/runtime/config identity.
- `Claude/Codex`: legacy compatibility and porting surfaces.

The highest leverage work is not adding features. It is separating product-level Galen code from legacy runtime substrate, then extracting repeated frontend/domain concepts into stable shared modules.

## Findings

### P0 - Canonical Product Identity Is Not Yet Enforced

Evidence:
- `README.md` describes Galen, but still tells users to configure `%USERPROFILE%\.claw\models.toml`.
- `rust/crates/galen/src-tauri/src/commands.rs` writes API keys to `~/.claw/models.toml`.
- `rust/crates/model-router/src/config.rs` reads `models.toml`, then `~/.claw/models.toml`.
- Windows scripts are named `Install-Galen*.ps1` / `Start-Galen.ps1`, but still target `claw.exe`.
- `AGENTS.md` and `CLAUDE.md` still reference `.claw.json` / `.claw/settings.local.json`.

Risk:
- Users see Galen but configure Claw.
- Future code will keep branching between product identity and runtime identity.
- Renames become harder after more UI and onboarding are built.

Recommended action:
- Introduce a canonical Galen path layer: `~/.galen/models.toml`, `~/.galen/workspace.json`, `~/.galen/mcp_servers.json`.
- Keep `.claw` as read-only legacy fallback for one migration window.
- Update docs/scripts to write and launch `galen.exe`; do not expose `claw.exe` in Galen-facing flows.

### P0 - Frontend Domain Logic Is Buried In One Page Component

Evidence:
- `rust/crates/galen/src/components/ResearchWorkbench.tsx` is about 670 lines.
- It owns artifact classification, study-stage definitions, metric derivation, Agent prompt construction, clinical reasoning form state, markdown output rendering, and table UI.
- Key definitions are local to the component: `clinicalStages`, `metrics`, `buildAgentTasks`, `classifyArtifact`, `artifactLabel`.

Risk:
- UI changes can accidentally alter clinical workflow behavior.
- The same artifact/stage/prompt logic cannot be reused by mobile, future desktop panels, backend indexing, or tests.
- The component will keep growing as each Galen capability is added.

Recommended action:
- Create `src/domain/clinicalStudy.ts` for stage definitions, artifact classification, labels, metrics, and prompt builders.
- Keep `ResearchWorkbench.tsx` as a renderer/orchestrator only.
- Add focused tests for artifact classification and prompt generation once a test surface is in place.

### P1 - UI Primitives Are CSS Classes, Not Reusable Components

Evidence:
- `rust/crates/galen/src/App.css` is about 1813 lines.
- Button, panel, status, section heading, workflow row, dataset row, handoff row, and empty state styles are repeated as CSS conventions rather than component contracts.
- `App.tsx` still contains modal/welcome/command palette markup and inline style fragments.

Risk:
- Visual consistency depends on remembering class names.
- New screens will re-create slightly different panels/buttons/status pills.
- The recent whitespace cleanup can regress when more features are added.

Recommended action:
- Add a small UI layer: `Panel`, `SectionHeading`, `StatusPill`, `ActionList`, `EmptyState`, `CommandButton`.
- Move layout-specific classes into workbench CSS; move reusable primitives into a smaller design-system CSS file.
- Keep the component API boring and product-specific; avoid a broad generic design system.

### P1 - Mode Definitions Are Duplicated Across Frontend And Rust

Evidence:
- `src/hooks/useMode.ts` defines `ChatMode`, labels, descriptions, and mode order.
- `src-tauri/src/modes.rs` defines the Rust enum, labels, descriptions, permission behavior, and system prompts.

Risk:
- A mode label, behavior, or availability can drift between UI and backend.
- Frontend can show a mode that backend treats differently.

Recommended action:
- Keep Rust as source of truth for mode capability and prompt behavior.
- Add a `get_modes` Tauri command returning id, label, description, and write permission.
- Frontend should only render what backend reports.

### P1 - Galen Has Its Own Tool/MCP Runtime Beside The Shared Runtime

Evidence:
- `src-tauri/src/tools/mod.rs` defines Galen's own `Tool`, `ToolContext`, `ToolRegistry`, write-tool gate, and MCP tool handling.
- `src-tauri/src/mcp_client.rs` implements Galen MCP client/config.
- The wider workspace already contains `runtime` and `tools` crates with MCP/tool registry behavior.

Risk:
- Tool permissions, MCP naming, connection lifecycle, and error handling will diverge.
- Fixes in the shared runtime will not automatically apply to Galen.
- Galen can become a parallel runtime rather than a product shell on top of shared capabilities.

Recommended action:
- Do not immediately import the whole shared runtime into Galen.
- First define a narrow `GalenCapability` boundary: medical tools, workspace tools, command execution, MCP bridge.
- Then decide which pieces can delegate to `runtime`/`tools` without pulling in CLI-specific behavior.

### P1 - Clinical Reasoning Is Exposed Through Two Entry Points

Evidence:
- `commands.rs` exposes `analyze_clinical_case` directly to the UI.
- `tools/clinical.rs` exposes the same capability to the model tool registry.
- Both call `medical_core::clinical::{analyze_case, format_report}`.

Risk:
- Schema, validation, output format, and safety copy can drift.
- UI and Agent may disagree on how the same capability behaves.

Recommended action:
- Create one Galen clinical reasoning service wrapper used by both command and tool entry points.
- Keep `medical-core` as the domain engine; keep Tauri/tool schema at Galen boundary.

### P2 - Documentation Surfaces Mix Product Docs And Legacy Runtime Docs

Evidence:
- `README.md` is Galen-facing.
- `USAGE.md`, `rust/README.md`, `docs/container.md`, `ROADMAP.md`, `PHILOSOPHY.md`, and Python parity files still describe Claw/Claude/Codex runtime work.

Risk:
- New contributors cannot tell what is product code versus inherited infrastructure.
- User docs leak implementation history.

Recommended action:
- Split docs into:
  - `docs/galen/` for product-facing desktop app docs.
  - `docs/runtime-legacy/` for inherited Claw runtime/CLI docs.
- Root README should link to Galen first and describe runtime docs as legacy substrate.

## Recommended Execution Order

1. Canonicalize Galen identity and paths: `.galen`, `galen.exe`, Galen docs/scripts, legacy `.claw` fallback.
2. Extract Galen domain config from `ResearchWorkbench.tsx`.
3. Add small reusable UI primitives and reduce `App.css` surface.
4. Move prompts/capability schemas behind stable Galen service boundaries.
5. Audit whether Galen tools can delegate to shared `runtime`/`tools` crates without inheriting CLI product assumptions.

## Non-Goals For The Next Pass

- Do not rewrite the whole runtime.
- Do not remove legacy Claw compatibility before adding Galen fallbacks/migration.
- Do not create a broad design system before the Galen workbench shape stabilizes.
- Do not merge Kinestra concepts into Galen.
