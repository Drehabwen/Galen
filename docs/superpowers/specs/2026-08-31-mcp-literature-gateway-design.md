# Galen MCP Literature Gateway Design

## Goal

Turn Galen's existing generic stdio MCP client into a traceable literature-provider gateway that can safely expose multiple academic MCP servers without tool-name collisions, preserve provider configuration, and record every literature search as durable project state.

## First vertical slice

This slice integrates three international providers and one experimental Chinese provider:

- Semantic Scholar via `s2-mcp-server`, built in and enabled by default;
- Crossref via `@cyanheads/crossref-mcp-server`, built in and enabled by default;
- PubMed remains the built-in authoritative biomedical provider and is represented in the same coverage model;
- CNKI Enhanced MCP is a built-in enabled connector when installed; login credentials are entered only in its visible browser and its coverage remains failed/unsearched until a real search succeeds.

This slice does not implement systematic-review screening, PRISMA, full-text synthesis, Zotero synchronization, automatic CNKI downloads, or FMS/subject context.

## Architecture

### MCP transport

Keep the existing JSON-RPC 2.0 stdio client. Extend `McpServerConfig` with an optional `env` map and pass it only to the spawned child process. Secrets remain in the user-scoped Galen config and are never committed or injected into model-visible tool arguments.

Every discovered external tool receives a collision-safe public name:

```text
mcp__<server-id>__<tool-name>
```

Server and tool segments are normalized to ASCII letters, digits, `_`, and `-`. Execution resolves the server explicitly; it must never search all servers for the first matching tool. Legacy `mcp__<tool-name>` calls remain supported only when exactly one connected server exposes that tool. Ambiguous legacy calls fail with a clear error.

### Recommended provider configuration

Galen owns a catalog of recommended MCP server entries. On startup it merges missing entries into `%APPDATA%/galen/mcp_servers.json` without changing existing commands, arguments, environment variables, or enabled flags.

Recommended entries:

- `semantic-scholar`: `uv tool run s2-mcp-server`, enabled by default;
- `crossref`: `deno run --allow-net --allow-env --allow-sys --allow-write npm:@cyanheads/crossref-mcp-server`, enabled by default;
- `cnki`: `cnki-enhanced-mcp`, enabled by default; Galen preserves an installed absolute command and its Playwright/profile environment.

PubMed is built in and therefore does not require a child process.

Galen starts the two international providers automatically through its bundled runtimes. Users may explicitly disable or override either entry, and catalog migration preserves that choice. A missing runtime, executable, credential, or upstream quota is reported as unavailable or failed, never as a zero-result search.

### SearchRun ledger

Add a workspace-scoped append-only ledger under the active research task:

```text
.galen/tasks/<task-id>/search-runs.jsonl
```

Each record contains:

- stable run ID;
- task ID;
- provider/server ID;
- tool name;
- normalized query text;
- original arguments;
- start and finish timestamps;
- status: `succeeded`, `failed`, or `partial`;
- result count when the provider exposes it;
- error message when applicable;
- raw-result content hash, not the full raw response.

The host writes the record after every recognized literature-search MCP call and after built-in PubMed searches. Tool failures are recorded before the error returns to the model.

Recognized search tools are declared in a provider catalog instead of guessed from arbitrary output.

### Coverage model

Coverage is computed from configured providers plus SearchRun history. A provider is one of:

- `searched`: at least one successful run for the active task;
- `failed`: the latest attempted run failed;
- `connected_not_searched`;
- `configured_disabled`;
- `unavailable`;
- `not_configured`.

Zero results is represented as a successful search with `resultCount = 0`; it is never conflated with unavailable, disabled, or failed.

The frontend receives coverage through a Tauri command and initially displays it in the existing evidence/inspector surface. Model context receives a compact coverage statement so generated claims must say “based on the searched providers” and list unsearched sources.

## Data flow

```text
Research request
  -> built-in PubMed or qualified MCP tool
  -> provider response
  -> SearchRun append
  -> provider result returned to model
  -> coverage recomputed
  -> Evidence may be created by the existing evidence workflow
```

SearchRun records retrieval provenance. Evidence records claims. They are intentionally separate: a search result is not automatically treated as evidence for a claim.

## Error handling

- Spawn, timeout, protocol, authentication, and parsing failures retain their existing typed MCP errors and also create failed SearchRuns for recognized searches.
- Duplicate server/tool names cannot silently route to the wrong server.
- Invalid environment variable names are rejected when loading configuration.
- Existing user configuration is backed up before a migrated file is replaced.
- The MCP gateway never reports “no evidence” from a disabled, unavailable, or failed provider.

## Security and privacy

- Provider secrets remain in the user config and child-process environment.
- Status APIs return environment variable names only, never values.
- CNKI credentials are entered in its visible browser and never passed as model arguments.
- CNKI is enabled on Windows, while coverage remains failed/unsearched until an end-to-end search succeeds; credentials and profile data stay outside the repository.

## Testing

- Unit tests for qualified tool naming, legacy ambiguity rejection, config environment handling, and non-destructive catalog merging.
- Unit tests for SearchRun append/load, zero-result success, failures, and coverage state derivation.
- Tool-registry tests proving two servers may expose the same tool without collision.
- Frontend tests for coverage labels and the distinction between zero results and unsearched.
- Integration smoke test using a deterministic local MCP fixture before testing public providers.

## Acceptance criteria

1. Galen can connect two MCP servers that both expose `search_papers` and invoke each explicitly.
2. API keys can be supplied through user-scoped MCP config without appearing in tool schemas, status responses, logs, or repository files.
3. Semantic Scholar and Crossref appear as built-in enabled providers on a fresh configuration and start without manual setup.
4. Every recognized external literature search and built-in PubMed search creates one durable SearchRun.
5. Coverage distinguishes searched-zero-results, failed, disabled, and never searched.
6. A failed or disabled Chinese provider causes a coverage limitation, not a “no Chinese evidence” conclusion.
7. Existing MCP configurations continue to load and are never overwritten during catalog migration.
