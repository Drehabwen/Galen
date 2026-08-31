# Galen Literature MCP / CNKI Handoff

Date: 2026-08-31 (Asia/Shanghai)

## Mission

Continue until Galen can execute a real CNKI literature search and return Chinese paper metadata. Do not report CNKI as searched merely because the MCP process connects or exposes tools.

## Repository state

- Repository: `D:\DEV\Galen-new`
- Maintained branch: `galen-research-workbench`
- Do not develop on `main`.
- The user explicitly requested working in the current checkout without a worktree.

Relevant commits:

```text
7f47dc1 feat(galen): enable CNKI literature provider
fc636f7 feat(galen): enable built-in literature MCP providers
ed99699 docs(galen): design MCP literature gateway
4c6345c fix(galen): render PDF artifacts with correct MIME
```

Untracked output directories predate this handoff and must not be staged:

```text
output/e2e-artifact-loop-v2/
output/real-task-evidence/
```

## What is implemented

Galen's MCP client now supports:

- user-scoped child-process environment variables;
- non-destructive built-in provider catalog migration;
- Semantic Scholar and Crossref enabled by default;
- CNKI Enhanced MCP represented as the enabled built-in `cnki` provider;
- collision-safe names such as `mcp__crossref__crossref_search_works`;
- exact server routing and ambiguous legacy-name rejection;
- standard MCP camelCase fields (`protocolVersion`, `inputSchema`, `isError`);
- JSON-RPC notifications without an `id` before the actual response;
- MCP tool errors returned as errors rather than successful zero-result text;
- 120-second MCP request timeout for browser automation.

Primary files:

```text
rust/crates/galen/src-tauri/src/mcp_client.rs
rust/crates/galen/src-tauri/src/tools/mod.rs
docs/superpowers/specs/2026-08-31-mcp-literature-gateway-design.md
docs/superpowers/plans/2026-08-31-default-literature-mcp.md
```

## Verified provider state

### PubMed

- Built-in direct client works.
- A real rehabilitation query returned 5/5 fetched records with PMID, title, authors, abstract, DOI, MeSH and publication types.

### Crossref

- MCP connects successfully.
- Seven tools discovered.
- Real search succeeded for `stroke home-based upper limb rehabilitation adherence`.
- First returned result concerned an upper-limb home telerehabilitation device for post-stroke patients.

### Semantic Scholar

- MCP connects successfully.
- Fourteen tools discovered.
- Anonymous search currently receives an upstream rate-limit error.
- Optional private configuration variable: `SEMANTIC_SCHOLAR_API_KEY`.
- Do not represent rate limiting as zero results.

### CNKI

- MCP package installed successfully.
- Galen connects successfully and discovers nine tools:
  `cnki_login`, `cnki_session_status`, `cnki_search`, `cnki_structured_search`, `cnki_get_metadata`, `cnki_download_paper`, `cnki_read_online_html`, `cnki_export_citations`, and `cnki_link_references`.
- A real CNKI search has **not succeeded yet**.

## CNKI local installation

External repository:

```text
D:\DEV\CNKI-Enhanced-MCP
```

Installed entry point:

```text
D:\DEV\CNKI-Enhanced-MCP\.venv\Scripts\cnki-enhanced-mcp.exe
```

Runtime data:

```text
D:\DEV\CNKI-Enhanced-MCP\.playwright-browsers
D:\DEV\CNKI-Enhanced-MCP\.cnki-data
```

The runtime data may contain browser cookies and must never be committed, copied into Galen, or printed to logs.

The external repository is intentionally dirty:

- `.venv/`, `.playwright-browsers/`, `.cnki-data/` and generated config are local runtime artifacts;
- `src/cnki_mcp/browser.py` has a local compatibility patch adding environment-controlled Playwright proxy and HTTPS-error options:
  `CNKI_MCP_PROXY` and `CNKI_MCP_IGNORE_HTTPS_ERRORS`.

Do not commit that external patch into the Galen repository. If it remains necessary, either maintain it as a reviewed fork/patch or replace it with an upstream-supported configuration.

## User-scoped Galen configuration

Path:

```text
C:\Users\labops\AppData\Roaming\galen\mcp_servers.json
```

The `cnki` entry is enabled and points at the installed executable. Its environment contains only names/paths and local network settings. Never print values for future credential-bearing variables.

Expected non-secret CNKI variables:

```text
PLAYWRIGHT_BROWSERS_PATH
CNKI_MCP_DATA_DIR
CNKI_MCP_BROWSER_CHANNEL=msedge
CNKI_MCP_PROXY=http://127.0.0.1:7890
CNKI_MCP_IGNORE_HTTPS_ERRORS=true
```

## Current blocker and root cause

The local proxy listener belongs to `iKuuuVPNCore` on `127.0.0.1:7890`.

Observed behavior:

1. With no explicit Playwright proxy, `www.cnki.net` resolves into the VPN fake-IP range (`198.18.0.x`) and navigation fails with `ERR_CERT_COMMON_NAME_INVALID` or times out.
2. With the explicit local proxy and HTTPS-error bypass, the certificate failure disappears, but CNKI EdgeOne rejects access (HTTP 418 / `ERR_HTTP_RESPONSE_CODE_FAILURE`).
3. The in-process CNKI smoke test completed with `status=partial`, zero results, and four failed advanced-search cells. This is a failed search, not a successful zero-result search.

Required user action: temporarily disconnect iKuuu VPN or add a direct rule:

```text
DOMAIN-SUFFIX,cnki.net,DIRECT
```

Do not terminate the user's VPN process without explicit permission.

After the user says the VPN/direct rule is ready, remove the explicit `CNKI_MCP_PROXY` and probably `CNKI_MCP_IGNORE_HTTPS_ERRORS` overrides from the user config before retesting, so CNKI uses a genuine direct TLS connection.

## Next execution steps

1. Confirm iKuuu VPN is disconnected or the CNKI DIRECT rule is active.
2. Verify DNS no longer resolves CNKI to `198.18.0.x`:

   ```powershell
   Resolve-DnsName www.cnki.net
   Resolve-DnsName kns.cnki.net
   ```

3. Remove proxy/certificate overrides from the `cnki.env` object in the user-scoped config, preserving executable and data paths.
4. Run a direct CNKI smoke search:

   ```powershell
   $env:PLAYWRIGHT_BROWSERS_PATH='D:\DEV\CNKI-Enhanced-MCP\.playwright-browsers'
   $env:CNKI_MCP_DATA_DIR='D:\DEV\CNKI-Enhanced-MCP\.cnki-data'
   $env:CNKI_MCP_BROWSER_CHANNEL='msedge'
   & 'D:\DEV\CNKI-Enhanced-MCP\.venv\Scripts\python.exe' `
     'D:\DEV\CNKI-Enhanced-MCP\tests\smoke_mcp_search.py' `
     '卒中 居家 上肢康复 依从性'
   ```

5. A successful response must contain at least one real paper/result and no navigation warning. `status=partial` with zero results does not pass.
6. Run the same search through Galen's `McpServerRegistry`, proving the public qualified tool path reaches `cnki_search` and survives the 120-second timeout.
7. If a login browser opens, let the user type credentials directly. Never request or relay passwords through the model.
8. Record CNKI coverage as searched only after the real Galen-mediated search succeeds.

## Verification commands

From `D:\DEV\Galen-new\rust`:

```powershell
cargo test -p galen --lib
cargo check --workspace
```

From `D:\DEV\Galen-new\rust\crates\galen`:

```powershell
npx tsc --noEmit
```

Last verified results before this handoff:

- Galen backend: 146/146 tests passed before the CNKI catalog update.
- MCP-focused suite after the CNKI catalog update: 17/17 passed.
- `cargo check -p galen`: passed.
- Earlier `cargo check --workspace`: passed.
- Frontend `npx tsc --noEmit`: passed before the CNKI catalog-only update (no frontend files changed afterward).

The full workspace test suite has unrelated pre-existing Windows failures in the `runtime` crate: 445 passed and 27 failed, mainly Unix/bash and Python fixture assumptions. Do not attribute those failures to this MCP change, but do not claim the entire workspace suite is green.

## Acceptance criteria

Do not close this work until all are true:

- `cnki` connects through Galen and exposes nine tools;
- a real `cnki_search` returns at least one Chinese paper;
- result titles and CNKI detail links are visible in the returned payload;
- login/captcha, if required, is completed by the user in a visible browser;
- failed, unavailable, unsearched and successful-zero-results remain distinct states;
- no cookies, passwords, API keys or browser-profile files enter Git.
