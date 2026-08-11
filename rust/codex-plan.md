# Task Plan

## Objective
- Expose Galen-MedX clinical reasoning as a direct product feature inside the Galen desktop app.

## Constraints
- Existing frontend files are already modified; preserve the current clinical workbench direction.
- Keep the first direct UI implementation deterministic and testable before adding full RAG/model orchestration.
- Preserve the current Tauri chat/tool architecture.

## Steps
- [completed] Add clinical reasoning types and deterministic engine in `medical-core`.
- [completed] Register a Galen tool that exposes the engine to the Agent loop.
- [completed] Update system prompt/tool guidance so the model can call the new tool.
- [completed] Run targeted Rust tests/checks.
- [completed] Add a Tauri command for direct clinical reasoning calls.
- [completed] Add a clinical reasoning panel to the React workbench.
- [completed] Add focused UI styling without changing unrelated frontend behavior.
- [completed] Run targeted Rust and frontend verification.
- [completed] Record outcome and remaining follow-up.
- [completed] Promote clinical reasoning to the first-screen work surface.
- [completed] Add responsive chat behavior for narrow app/browser widths.
- [completed] Clarify direct reasoning availability in browser preview versus desktop backend.
- [completed] Rebuild and verify the updated UI.

## Verification
- `cargo test -p medical-core clinical`
- `cargo check -p galen`
- `npm run build`

## Outcome
- Added `medical-core::clinical` with structured findings, risk signals, candidate disease ranking, information gaps, markdown report formatting, and unit tests.
- Added Galen tool `analyze_clinical_case` and registered it in the existing tool registry.
- Updated the Galen system prompt so symptom/case/differential-diagnosis requests route to the new tool first.
- Added direct Tauri command `analyze_clinical_case` for product UI calls.
- Added a clinical reasoning panel in the React workbench with case input, age/sex metadata, markdown report rendering, and Agent review handoff.
- Verified with `cargo test -p medical-core clinical`, `cargo check -p galen`, and `npm run build`.
- Started Vite preview at `http://127.0.0.1:5174`.
- Promoted clinical reasoning to the first-screen work surface, added browser-preview/backend status copy, and collapsed Chat into a compact bottom input bar on narrow widths.
- Re-verified with `npm run build` and browser checks at the current in-app browser width.
- Follow-up: connect local guideline/textbook/case RAG and add dedicated evidence retrieval to each candidate diagnosis.
