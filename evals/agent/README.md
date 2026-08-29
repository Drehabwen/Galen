# Galen Agent Evals

This directory is an external evaluation layer. It does not become part of the
Galen desktop runtime.

The integration combines four ideas:

- **Inspect AI** supplies task execution, transcripts, limits, logs and scorers.
- **tau-style domain simulation** separates the simulated user's private goal
  from the state and policy visible to Galen.
- **Letta-style memory probes** test retention, correction, temporal precedence
  and cross-session recovery.
- **Galen's Rust evaluator** remains the authority for files, tool traces,
  medical facts, latency, token usage and negative-optimization gates.

## Setup

```powershell
cd evals/agent
python -m venv .venv
.venv\Scripts\python -m pip install -r requirements.txt
.venv\Scripts\python -m pip install -e . --no-deps
```

Build Galen's native evaluator once:

```powershell
cd ../../rust
cargo build -p galen --bin eval
```

Validate the suite without calling a model:

```powershell
cd ../evals/agent
.venv\Scripts\inspect eval galen_agent_eval/tasks.py@galen_contracts `
  --model mockllm/model --limit 1
```

Run `galen_agent_eval/tasks.py@galen_foundation` for real cases. Real runs use
the model configured in `~/.galen/models.toml`. Set
`GALEN_EVAL_MODEL` to compare a specific Galen model alias. Inspect's model is
only a scheduler for this adapter; it does not replace Galen's own model router.

## Boundaries

The simulated user never receives `private_goal`, `hidden_facts` or the gold
final state. The scorer reads the immutable native JSONL record produced by
Galen. LLM-judged fluency can be added later, but it can never override failed
medical, state-integrity or artifact gates.

The simulated user is response-aware. It branches to tool recovery, memory
challenge, delivery demand, in-app preview demand, latency challenge, or
acceptance based only on observable outcomes. Hidden facts never appear in its
transcript; Inspect stores the complete trace in scorer metadata.

## Browser journey

The browser journey uses a query-gated Tauri fixture (`?e2e=1`) so the React UI
can run in Chromium without weakening the desktop backend. It covers case
import, human review, derived-state update, golden journeys, native benchmark
display, screenshots, console errors and a Playwright trace.

```powershell
powershell -ExecutionPolicy Bypass -File scripts/evals/run_galen_ui_e2e.ps1
```

Artifacts stay local under `rust/crates/galen/output/playwright/`. The fixture
cannot replace an existing Tauri runtime and is inactive unless explicitly
enabled by the E2E query parameter.
