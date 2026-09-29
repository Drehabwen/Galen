# ScienceAgentBench ClinTox contract-aware competition

## Scope

This is a one-run native-agent diagnostic on verified ScienceAgentBench
instance 1, not a stable leaderboard. Galen, Codex, and Claude received the
same 3,571-byte prompt (`SHA-256 80396d20426c113e640447c8444c39f5c5cc0d3ce98a80b9746d1d1ac6fb1155`),
the `scienceagentbench-clintox-v1` contract, and the `deepseek-v4-pro` model
identifier. Generated programs were frozen before execution and were not
repaired. All programs were evaluated offline in the same calibrated CPU image
against the same official evaluator and verified data.

## Result

| Candidate | Generation time | Valid program | ROC-AUC | ≥ 0.77 | Official success | Versus 0.8398 baseline |
|---|---:|---:|---:|---:|---:|---:|
| ECFP logistic baseline | n/a | 1 | 0.839805 | yes | 1 | reference |
| Claude + DeepSeek V4 Pro | 256.303 s | 1 | 0.786782 | yes | 1 | -0.053023 |
| Codex + DeepSeek V4 Pro | 43.910 s | 1 | 0.228364 | no | 0 | -0.611441 |
| Galen + DeepSeek V4 Pro | 20.720 s | 0 | not evaluated | no | 0 | not comparable |

The official gold program also passed the 0.77 threshold during environment
calibration. The official evaluator does not expose its numeric gold ROC-AUC.

## Failure analysis

- Claude correctly selected `dc.models.MultitaskClassifier` and extracted
  `test_scores[:, task, 1]`, so its output preserved both task identities and
  positive-class probabilities. It is the only generated candidate that
  passed the official evaluator in this run.
- Codex produced a complete 292-row output in the correct order, but converted
  `(samples, tasks, classes)` predictions with `predictions[:, 0, :]`. Its two
  output columns therefore represented class 0 and class 1 of the first task,
  rather than positive-class probabilities for both tasks. This is a semantic
  tensor-shape error, not a filesystem or environment failure.
- Galen honored the authoritative `/testbed/benchmark/datasets/clintox` paths,
  but imported `MultitaskClassifier` from `deepchem.models.torch_models`, where
  it is unavailable in DeepChem 2.8.0. The supported target used by the gold
  calibration is `dc.models.MultitaskClassifier`; the program exited before
  training and produced no output.
- During Galen generation, six of seven tool calls failed because it tried to
  inspect eventual container paths from the temporary generation workspace.
  It recovered and wrote `program.py`, but this shows that Galen still mixes
  generation-time filesystem scope with execution-time contract paths.

## Comparability limits

- This is one generated program per framework. DeepChem training is stochastic,
  so it is sufficient for defect discovery but not for a stable ranking.
- Codex detected host skills despite the isolated wrapper and is therefore
  eligible only for the native-agent track, not the controlled-core track.
- Native orchestration is intentionally different: Galen uses its product tool
  loop, while Codex and Claude use their own CLIs. The shared model, prompt,
  program execution, data, and scoring controls isolate much of the comparison,
  but not orchestration implementation.

## Required next controls

1. Add an execution-environment capability manifest listing importable symbols
   and tensor/output contracts without revealing labels or gold results.
2. Add a generated-program static check for task/class axis handling and an
   output-schema smoke check before expensive training.
3. Repeat generation at least five times per agent with fixed generation
   parameters, then report pass@1, pass^5, ROC-AUC distribution, latency, tool
   errors, and cost. Do not promote a winner from this single run.
