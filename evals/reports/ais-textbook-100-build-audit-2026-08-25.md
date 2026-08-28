# AIS textbook 100-case dataset build audit

Date: 2026-08-25

## Outcome

The textbook case chapter was indexed from source case 1 through 100. The local,
private dataset contains 100 case records and 400 case-derived tasks. It expands
the existing 10-case pilot without treating OCR output as verified clinical
gold.

| Layer | Count | Scoring status |
| --- | ---: | --- |
| Visually verified pilot cases (21-30) | 10 | Eligible for exact hard gates |
| OCR candidate cases | 90 | Review required; no exact hard gates |
| Candidate numeric degree mentions | 483 | Non-scoring until verified |
| Tasks | 400 | Four per source case |
| Development / validation / hidden cases | 60 / 20 / 20 | Grouped by case ID |
| High / medium priority review cases | 22 / 68 | Human queue |

Dataset content SHA-256:
`05b0d4980aa6c74656ab413afde6cae0e7deda3eb5984cd18184e0d61343d771`

## Source and privacy boundary

- Source: `docs/脊柱侧弯保守治疗100例_14996973.pdf`
- Source SHA-256:
  `1b6631c313f4efeb04027aad1042f519b6933b8747a81f40e39c2ea6ed7c218e`
- The PDF, rendered pages and complete OCR text are excluded from Git.
- Complete OCR evidence and the generated 100-case dataset live under
  `evals/private/ais-textbook-100-v1/`.
- Tracked tooling stores no photographs or complete textbook passages.

## Index verification

The PDF page number is the printed book page plus 13 in the case chapter. The
scanner found all 100 headings and handled three non-default layouts:

- PDF page 87 contains separate starts for cases 27 and 28.
- PDF page 109 contains cases 44 and 45; local OCR read `案例44` as `案例4`.
  The correction was visually confirmed against the rendered page and is
  explicitly recorded by the scanner.
- Paired headings `案例89-90` and `案例91-92` expand to two case IDs that share
  their paired source block.

Visual checks were performed on PDF pages 44 (case 1), 109 (case 44/45), 134
(table-form case 63), and 172 (paired cases 89/90). These confirm that the
heading parser covers prose, same-page, table, and paired-case layouts.

## Automated validation

The builder rejects the dataset unless all of the following hold:

1. Case IDs exactly cover `AIS-C001` through `AIS-C100`.
2. Every case belongs to exactly one 60/20/20 grouped split.
3. Every case produces exactly four tasks and one matching input per task.
4. Every observation locator belongs to that case's source-page set.
5. OCR evidence hashes match the case records.
6. Only observations marked `verified` can enter a hard gate.
7. All 90 OCR candidate cases appear in the human review queue.

The current build passes all checks. Cases 3 and 4 have no reliably parsed
degree token and therefore contain an explicit pending observation rather than
a fabricated number. Twenty candidate cases have unresolved sex and sixteen
have no parsed birth year; these are routed to review instead of silently
imputed.

## Reproduce

Render the source pages to `tmp/pdfs/ais100-pages/`, then run:

```powershell
python scripts/evals/scan_ais_textbook_cases.py --start 30 --end 188
python scripts/evals/build_ais_textbook_100_dataset.py
```

The scanner checkpoints every page, so an interrupted OCR run resumes from its
cache. Parser changes rebuild the index from cached OCR without rescanning.

## What this dataset can test now

- The 10 verified cases can continue to measure exact extraction, temporal
  leakage, longitudinal synthesis and research-boundary reliability.
- The 90 candidate cases can immediately stress robustness, source grounding,
  missing-data behavior and refusal to invent facts.
- They must not yet be used to claim a 100-case exact clinical accuracy score.
  Promoting them to gold requires the queued visual review of numeric values,
  event labels, imaging state and signed angles.

## Next review order

Review the 22 high-priority cases first, especially table/figure-dominant cases
63-73, paired cases 89-92, the two cases with no degree token, and records with
unresolved demographics. Then review medium-priority numeric mentions in
batches, rerunning the builder after each frozen tranche. This produces useful
20-, 40-, 60-, and 100-case gold milestones without waiting for all 90 cases.
