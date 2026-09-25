# Public benchmark workspace

Local upstream checkouts, downloaded datasets, and virtual environments are
ignored. Reproducibility metadata is kept in `scienceagentbench-manifest.json`.

ScienceAgentBench preparation uses:

- upstream repository commit `c26e151ed601ba109dc4d35e057ff8e73fec469d`;
- Hugging Face config `default`, split `verified` (102 rows);
- parquet SHA-256 `C6F937863A220BD1762A00C20A0F79CC8DFCA900B819BDB552150310731AE147`;
- pinned local Codex and Claude Code versions from
  `../external-runtimes/package-lock.json`.

The public annotation sheet is available locally, but the official evaluator
datasets, gold programs, scoring rubrics, and evaluation programs are in the
access-controlled `benchmark_verified.zip`. Do not report an official score
until that archive is obtained lawfully and its contents are validated.

After authenticated download, place the archive at
`evals/public-benchmarks/downloads/benchmark_verified.zip`. The downloads
directory is ignored because the upstream project prohibits redistribution of
the unzipped evaluator data.

Run the deterministic preflight before any evaluation:

```powershell
python scripts/evals/scienceagentbench_preflight.py
```

`runtime_ready` only means the two adapters and public assets are present.
Only `official_score_ready` permits publishing a ScienceAgentBench score.

The evaluation has two explicit comparison tracks. `native_agent` permits each
framework to discover and use its normal skills because skill selection and
adaptation are part of product-level agent capability. `controlled_core`
matches external context as closely as possible to isolate orchestration from
ecosystem advantages. Windows Codex 0.157.0 currently discovers the real
`%USERPROFILE%/.agents/skills` directory even when its experimental skip flag
is enabled, so those runs remain valid for `native_agent` and are ineligible
only for `controlled_core`.
