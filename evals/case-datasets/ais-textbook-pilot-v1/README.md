# AIS textbook pilot dataset v1

This is a local-research evaluation dataset derived from cases 21-30 in
`脊柱侧弯保守治疗100例` (2021). It stores normalized facts, timepoints,
source locators, ambiguity and evaluation tasks. It does not store the source
PDF, page images, complete OCR text or patient photographs.

## Licensing and source boundary

The repository MIT license covers Galen's code, schemas, task definitions and
dataset tooling. It does not relicense the source textbook. Source pages,
figures, photographs and OCR text are intentionally excluded. The normalized
case facts and locators are provided as a research-evaluation pilot; users are
responsible for confirming that their use of the source material is permitted
in their jurisdiction and institution.

## Dataset boundary

- `cases.json` is the normalized source-of-truth layer.
- `tasks.jsonl` is generated from `cases.json`; do not edit it by hand.
- `inputs.jsonl` contains only the events visible to each task. The validator
  rejects hidden follow-up events that leak into an input record.
- `splits.json` groups every task from the same source case into one split.
- Only observations with `verification_status = "verified"` may become hard
  gates. `candidate` and `disputed` observations remain visible to reviewers
  but are excluded from exact scoring.
- Negative Cobb values are preserved as signed values when the source describes
  reverse correction/overcorrection.
- Book claims are not automatically treated as causal or guideline-level truth.

## Build and validate

```powershell
python scripts/evals/ais_textbook_dataset.py build
python scripts/evals/ais_textbook_dataset.py validate
python scripts/evals/ais_textbook_dataset.py export-galen --case AIS-C021
python scripts/evals/ais_textbook_dataset.py export-galen --split development
```

The validator checks source locators, event references, split leakage, disputed
facts, task counts and future-information leakage.

## Pilot composition

- 10 source cases: AIS-C021 through AIS-C030.
- Four tasks per case: baseline extraction, temporal safety, longitudinal or
  missing-data synthesis, and research-boundary reasoning.
- 40 generated tasks in total.
- 40 sanitized task inputs, separate from hidden gold and source images.
- Development/validation/hidden split is performed by source case, never by
  paraphrased task.

## Review workflow

1. Compare each observation against its PDF page and figure.
2. Change `candidate` to `verified` only after visual review.
3. Keep source disagreements as `disputed`; never silently resolve them.
4. Run `validate` before using the dataset as a baseline.
5. Freeze a reviewed version by recording its generated dataset hash in a
   release report.
