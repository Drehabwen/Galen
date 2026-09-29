# Galen ClinTox P0 remediation — DeepSeek V4.1 Flash

## Controls

- Display model: DeepSeek V4.1 Flash
- API model identifier: `deepseek-flash`
- Contract: `scienceagentbench-clintox-v2`
- Shared official CPU image, verified data, offline execution, and evaluator
- Generated programs frozen before execution; no candidate was repaired
- Codex and Claude generation workspaces were moved outside the repository
- Galen used an ephemeral `models.toml`; the user's global model configuration
  was not modified and the temporary credential-bearing configuration was
  deleted by its wrapper

The Claude adapter's first Flash attempt did not deliver the prompt as a user
message and produced no program. It is recorded as a transport failure and
excluded from agent scoring. The adapter was changed to pass the prompt through
stdin; the single technical retry reported canonical model `deepseek-flash`
and produced the frozen Claude candidate.

## One-shot result

| Candidate | Generation time | Internal delivery | Valid program | ROC-AUC | Official success |
|---|---:|---:|---:|---:|---:|
| ECFP logistic baseline (v1) | n/a | n/a | 1 | 0.839805 | 1 |
| Codex + DeepSeek V4.1 Flash | 87.685 s | n/a | 1 | 0.816971 | 1 |
| Claude + DeepSeek V4.1 Flash | 8.066 s | n/a | 0 | not evaluated | 0 |
| Galen + DeepSeek V4.1 Flash | 9.873 s | 1.000 / pass | 0 | not evaluated | 0 |

Codex passed the official 0.77 threshold and remained 0.022834 below the
deterministic baseline. Claude and Galen exited during ECFP conversion before
training, so neither has a ROC-AUC value in this run.

## What changed in Galen

- Contract v2 separates the code-generation workspace from future `/testbed`
  runtime paths and explicitly says not to probe runtime paths while generating.
- The capability manifest identifies `deepchem.models.MultitaskClassifier` as
  available and the incorrect PyTorch namespace symbol as unavailable.
- The prediction contract fixes tensor axes as `(sample, task, class)` and
  requires `predictions[:, :, 1]` for positive-class probabilities.
- Rust evaluator preview classification now treats Python and other source-code
  extensions as previewable. Galen's internal case changed from
  `hard_gates_passed=false`, quality 0.909, to `hard_gates_passed=true`, quality
  1.000.
- Galen requested and recorded model alias `deepseek-flash` through an isolated
  route whose resolved API model identifier is also `deepseek-flash`.

## Remaining failure

Both Claude and Galen directly converted the output of
`CircularFingerprint.featurize()` to a dense NumPy array. Three malformed
training SMILES produce empty/ragged feature values in DeepChem 2.8.0, causing
`ValueError: setting an array element with a sequence`. Codex generated a more
defensive featurization path and completed successfully.

This is not a model-routing, filesystem, import-symbol, or tensor-axis failure.
The next contract/runtime capability must specify a general invalid-molecule
policy: detect failed featurization, preserve test-row order, and use a
deterministic fallback or validated filtering strategy without label leakage.

## Interpretation

The P0 intervention improved Galen's orchestration and removed the exact two
defects found in the V4 Pro run, but did not yet produce an executable scientific
program. This remains a one-run defect-discovery result, not a leaderboard.
Do not begin five-repeat reliability testing until the invalid-input policy is
implemented and the same single instance passes once without manual candidate
repair.
