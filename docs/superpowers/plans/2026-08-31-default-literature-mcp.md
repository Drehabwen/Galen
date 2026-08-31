# Default Literature MCP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Semantic Scholar and Crossref start automatically as Galen-owned literature tools while preserving user configuration and keeping PubMed as the built-in fallback.

**Architecture:** Extend the current user-scoped MCP configuration with a non-destructive built-in provider catalog. On startup Galen merges missing catalog entries, enables the two international literature providers by default, launches them through bundled runtimes, and exposes every discovered tool through a collision-safe server-qualified name.

**Tech Stack:** Rust, Tokio stdio JSON-RPC, Serde, Deno, uv.

**Spec:** `docs/superpowers/specs/2026-08-31-mcp-literature-gateway-design.md`

## Global Constraints

- Work on `galen-research-workbench`; do not create a worktree because the user explicitly declined isolation.
- Preserve existing commands, arguments, environment maps, and enabled flags.
- Semantic Scholar and Crossref are default-enabled; CNKI remains experimental and disabled.
- Provider failure must not disable built-in PubMed or be represented as zero search results.

---

### Task 1: Built-in provider catalog and migration

**Files:**
- Modify: `rust/crates/galen/src-tauri/src/mcp_client.rs`
- Test: `rust/crates/galen/src-tauri/src/mcp_client.rs`

**Interfaces:**
- Produces: `McpConfig::with_builtin_catalog(existing) -> McpConfig`
- Produces: `McpConfig::load_or_initialize() -> Option<McpConfig>`
- Extends: `McpServerConfig.env: HashMap<String, String>`

- [ ] Write tests proving a fresh catalog enables `semantic-scholar` and `crossref`, leaves `cnki-experimental` disabled, and preserves an existing provider override.
- [ ] Run the focused tests and confirm they fail because catalog merge and environment support do not exist.
- [ ] Implement environment deserialization, runtime commands, and non-destructive catalog merge.
- [ ] Run the focused tests and confirm they pass.

### Task 2: Collision-safe built-in tool exposure

**Files:**
- Modify: `rust/crates/galen/src-tauri/src/mcp_client.rs`
- Modify: `rust/crates/galen/src-tauri/src/tools/mod.rs`
- Test: `rust/crates/galen/src-tauri/src/tools/mod.rs`

**Interfaces:**
- Produces public names in the form `mcp__<server-id>__<tool-name>`.
- Execution resolves the named server directly; legacy names work only when unambiguous.

- [ ] Write failing tests for qualified definition names and qualified-name parsing.
- [ ] Run the focused tests and confirm failure is caused by the old unqualified naming.
- [ ] Implement qualified discovery and dispatch with an ambiguity error for legacy names.
- [ ] Run the focused tests and confirm they pass.

### Task 3: Runtime verification

**Files:**
- Modify only if an integration defect is found: `rust/crates/galen/src-tauri/src/mcp_client.rs`

**Interfaces:**
- Consumes the migrated user configuration and bundled `deno`/`uv` runtime resolution.
- Produces connected Semantic Scholar and Crossref status plus discovered tools.

- [ ] Run all Galen backend tests.
- [ ] Launch both configured providers through Galen's MCP connection function and inspect discovered tool counts.
- [ ] Execute one real search against each connected provider.
- [ ] Run `cargo check --workspace`, `cargo test --workspace`, and `npx tsc --noEmit`.
- [ ] Review `git diff` and confirm no user output directories or secrets are staged.
