# Task Plan

## Objective
- Restore Galen startup so the Tauri window can load the local frontend instead of showing `ERR_CONNECTION_REFUSED`.

## Constraints
- Worktree is dirty; do not revert unrelated user or agent changes.
- Galen uses Tauri 2 + React/Vite under `rust/crates/galen`.
- Prefer targeted fixes and verify with build/startup commands.

## Steps
- [in_progress] Inspect Tauri/Vite startup configuration and confirm the failure mode.
- [pending] Run frontend type/build checks to catch compile-time startup blockers.
- [pending] Patch the minimal config or code issue causing the refused localhost load.
- [pending] Verify Galen can serve/load the frontend and record residual risk.

## Verification
- `npm run build` or `npx tsc --noEmit` in `rust/crates/galen`.
- Tauri/Vite dev startup enough to confirm the configured localhost endpoint is listening.

## Outcome
- Pending.
