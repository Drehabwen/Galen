# Galen ClinTox invalid-molecule remediation — DeepSeek V4.1 Flash

## Controls

- Model alias and API model identifier: `deepseek-flash`
- Contract: `scienceagentbench-clintox-v3`
- Shared calibrated CPU image: `sab.eval.x86_64.1:gold`
- One untouched generated candidate; no manual repair
- Candidate SHA-256: `7c38c989971c2c10c3fc0efee3a10109c6ca32516cec1f4ff712959838fe87cc`
- Contract SHA-256: `3f7cfcab5b7ce575cb71e85c3dc2c91e7089c137a9676c9642130fb1bbc1cf02`

## Result

| Candidate | Valid program | ROC-AUC | 0.77 threshold | Official success |
|---|---:|---:|---:|---:|
| ECFP logistic baseline | 1 | 0.839805 | pass | 1 |
| Galen + DeepSeek V4.1 Flash, contract v3 | 1 | 0.794881 | pass | 1 |

The official evaluator reported both data correctness and function correctness.
The program preserved all 292 test rows. Runtime diagnostics reported three
training fallbacks out of 1,192 rows and zero test fallbacks out of 292 rows.

## What was fixed

Contract v3 turns malformed-molecule handling into a submission gate: each row
must become a fixed 1024-wide float32 ECFP vector; failed, empty, or malformed
features must end in a deterministic all-zero vector; no row may be dropped or
reordered; fallback behavior cannot depend on labels; and train/test fallback
counts must be logged separately to stderr.

A standard-library static preflight now blocks expensive training when a
candidate lacks verified classifier symbols, positive-class tensor extraction,
fixed-width fingerprints, deterministic fallback, row preservation, or fallback
observability. The generated Galen candidate passed all ten checks before it was
executed.

## Interpretation

This is a real P0 correction: Galen moved from `valid_program=0` with no score to
an officially valid program above the 0.77 threshold. Its ROC-AUC remains
0.044924 below the deterministic baseline and 0.022090 below the earlier Codex
Flash one-shot result. One run is not a reliability or architecture ranking.

The next evaluation step is repeated seeded generation/execution after the
remaining Galen tool-delivery errors are reduced; the successful internal run
still recorded 11 tool errors in 15 calls despite producing a valid artifact.
