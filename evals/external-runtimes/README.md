# External agent runtimes

This directory contains version-pinned, project-local agent CLIs used only by
controlled evaluations. `node_modules/` and `config/` are ignored. Never put an
API key in `package.json`, a committed config, a command-line argument, a run
record, or raw events.

Install exactly the locked dependencies:

```powershell
npm ci --prefix evals/external-runtimes
```

Expected executables:

- `node_modules/.bin/codex.cmd`
- `node_modules/.bin/claude.cmd`

Formal comparisons use temporary per-run configuration and runtime-only secret
injection. User-global Codex and Claude settings are not part of the benchmark.
The wrapper accepts a cached official DeepSeek Codex setup catalog when present;
otherwise it downloads the official script and rejects versions other than 1.4.0.
