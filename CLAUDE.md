# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Repository overview

- **Galen** — Tauri 2.x desktop workbench (`rust/crates/galen/`). A closed-loop workbench for rehabilitation research: data acquisition, processing, analysis, report writing, and human sign-off.
- **Active branch** — `galen-research-workbench` is the only maintained branch. `main` is a historical import; do not develop there.
- **Shared crates** — `api`, `runtime`, `tools`, `plugins`, `model-router`, `medical-core`, `telemetry`, `commands`.

## Stack

- Backend: Rust workspace at `rust/`, Tauri 2.x
- Frontend: React 18 + TypeScript + Vite 5 (`rust/crates/galen/src/`)
- Design system: CSS custom properties (`rust/crates/galen/src/styles/`), palette: warm white #faf7f2 / ink green #1b2e1f / lake orange #d4743c
- Sidecars: typst / deno / uv, downloaded per-platform by `rust/scripts/download_sidecars.py`
- Models: DeepSeek V4 Pro (default) / V4 Flash via `~/.galen/models.toml`; no Anthropic dependency

## Key files

- `rust/crates/galen/src-tauri/src/backend.rs` — chat loop + tool execution + medical persona injection
- `rust/crates/galen/src-tauri/src/personas.rs` — persona / role definitions
- `rust/crates/galen/src-tauri/src/skills.rs` — research taste criteria + assembled skill library
- `rust/crates/galen/src-tauri/src/tools/rehab.rs` — read-only rehab data tool (SQLite)
- `rust/crates/medical-core/src/pubmed.rs` — PubMed search client (DTD-tolerant)
- `rust/crates/galen/src/App.tsx` — frontend task-loop state machine
- `rust/crates/galen/src/components/SessionChat.tsx` — session auto-execution loop

## Verification

- Rust: `cd rust && cargo check --workspace && cargo test --workspace`
- Clippy: `cargo clippy --workspace --all-targets -- -D warnings`
- Frontend: `cd rust/crates/galen && npx tsc --noEmit`

## Working agreement

- Prefer small, reviewable changes; verify (build/test/run) before claiming completion.
- Product config lives under `~/.galen/`; do not commit personal keys or local DB paths.
- Design changes must reference `tokens.css` variables, never hardcoded colors.
- This repo is a research workbench, not a generic assistant: keep changes aligned with the rehab research closed-loop direction.
- Do not overwrite this file automatically; update it intentionally.
