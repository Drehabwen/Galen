# Upstream evaluation architecture

Galen integrates these projects at the evaluation boundary. No upstream source
code is vendored or copied into the product runtime.

| Upstream | Galen use | Integration status |
|---|---|---|
| [Inspect AI](https://github.com/UKGovernmentBEIS/inspect_ai) | Task orchestration, isolated sample state, transcripts, limits and scorer aggregation | Runtime dependency pinned to `0.3.260` in this directory |
| [tau3-bench](https://github.com/sierra-research/tau2-bench) | Domain policy, simulated-user private goal, visible opening and verifiable final environment state | Scenario contract adapted locally; upstream package is not a runtime dependency |
| [Letta Evals](https://github.com/letta-ai/letta-evals) | Retention, reach-back, correction and cross-session memory probes | Memory scoring protocol adapted to Galen's native context record |
| [Harbor](https://github.com/harbor-framework/harbor) | Future parallel model/agent comparison in isolated environments | Deferred until the suite has enough costly repeated runs |
| [OSWorld V2](https://github.com/xlang-ai/OSWorld-V2) | Future full desktop interaction and final UI-state verification | Playwright remains the lighter local first step |

## Why only Inspect is installed

Installing several competing agent runtimes would make the benchmark change the
system it is meant to measure. Galen therefore installs one neutral orchestrator
and adopts data contracts from the other projects. The native Rust loop remains
the agent under test, and its JSONL record remains the source of truth.

An upstream update is never automatic. Inspect version changes can alter scores,
logs or execution semantics, so the pinned version must be updated together with
a baseline re-run and a benchmark version bump.
