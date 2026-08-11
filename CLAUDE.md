# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Repository overview
- **Galen** — Tauri 2.x desktop workbench (`rust/crates/galen/`). AI-driven code + research assistant.
- **Claw CLI** — terminal coding agent (`rust/crates/rusty-claude-cli/`). Separate product, shared infrastructure.
- **Shared crates** — `api`, `runtime`, `tools`, `plugins`, `model-router`, `medical-core`, `telemetry`.

## Stack
- Backend: Rust (workspace at `rust/`), Tauri 2.x
- Frontend: React 18 + TypeScript + Vite 5 (`rust/crates/galen/src/`)
- Design system: CSS custom properties (`rust/crates/galen/src/styles/`)
- Palette: warm white #faf7f2 / ink green #1b2e1f / lake orange #d4743c

## Verification
- Rust: `cd rust && cargo check --workspace && cargo test --workspace`
- Clippy: `cargo clippy --workspace --all-targets -- -D warnings`
- Frontend: `cd rust/crates/galen && npx tsc --noEmit`

## Key files
- `rust/crates/galen/src/styles/tokens.css` — design tokens (colors, type, spacing)
- `rust/crates/galen/src-tauri/src/backend.rs` — chat loop + tool execution
- `rust/crates/galen/src-tauri/src/personas.rs` — persona/role definitions
- `rust/crates/galen/src-tauri/src/modes.rs` — mode definitions (Discuss/Plan/Auto)
- `rust/crates/galen/src/domain/` — frontend domain logic (project detection, clinical)

## Working agreement
- Prefer small, reviewable changes.
- Galen product code uses `~/.galen/` paths; `~/.claw/` is read-only fallback for claw CLI.
- Design changes must reference `tokens.css` variables, never hardcoded colors.
- Do not overwrite this file automatically; update it intentionally.
