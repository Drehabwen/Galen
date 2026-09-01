# Nexus / DeepRehab Core integration design

## Decision

`Rehab` is the future clinical rehabilitation workbench, renamed in product
language as **Nexus / DeepRehab Core**. It owns longitudinal rehabilitation
context. Galen remains a separate research-and-evidence product that connects
to Nexus through a small, auditable contract.

This decision does not merge repositories, copy patient data into Galen, or
replace any existing product. It establishes the boundaries needed for a
single coherent product line.

## Product roles

| System | Owns | Does not own |
| --- | --- | --- |
| Nexus / DeepRehab Core (`Rehab`) | case context, assessments, clinical records, reports, follow-up state | literature search or research-project execution |
| RehabScreenLab | standardized field capture and screening-session evidence | longitudinal case management |
| Galen | research projects, literature coverage, evidence packets, research deliverables | identifiable patient records or treatment decisions |
| RehabGPT- / XiaoZhu | family-facing plan execution and adherence feedback | clinician source of truth |

## Core record model

Nexus shall become authoritative for four objects:

```text
Case
 └─ Assessment
     ├─ Observation / Measurement
     ├─ Evidence item
     └─ Report
```

- `case_id`: stable Nexus identifier; never sent to Galen in raw form.
- `assessment_id`: a specific assessment session, including time, protocol and
  quality state.
- `evidence_item_id`: one measurement, observation, voice-derived field or
  source artefact with provenance.
- `report_id`: a versioned output that lists the assessment and evidence used.

Every cross-system request uses an opaque `research_context_id`, not an
identifying case ID.

## First integration contract

Nexus exposes a manually approved, de-identified research context:

```json
{
  "research_context_id": "rcx_...",
  "assessment_id": "asm_...",
  "question": "...",
  "population_summary": "de-identified cohort/case characteristics",
  "measurements": [{"metric": "...", "value": 0, "unit": "...", "source": "..."}],
  "provenance": [{"evidence_item_id": "...", "captured_at": "...", "quality": "..."}]
}
```

Galen returns an evidence packet, not a clinical command:

```json
{
  "research_context_id": "rcx_...",
  "claims": [{"text": "...", "citation_links": ["https://..."]}],
  "coverage": [{"provider": "PubMed", "status": "searched"}],
  "artifacts": [{"name": "...", "url": "...", "mime": "application/pdf"}]
}
```

Nexus displays this output as research support with its source coverage and
links intact. It does not automatically turn it into a prescription or
patient-facing recommendation.

## Plugin boundaries

Nexus keeps its plugin shape, but plugins must read/write the four core
objects rather than storing isolated state:

- **Vision3 / motion** creates assessment measurements and raw-evidence links.
- **MedVoice** creates structured observations and provenance for transcript
  source.
- **Galen connector** reads only a user-approved de-identified context and
  registers returned evidence packets and artifacts.
- **XiaoZhu connector** receives an approved family plan and returns adherence
  events, never edits clinician-originated findings.

## Migration sequence

1. Audit the existing `Rehab` Nexus source and remove build artefacts and
   Python cache files from source control in a dedicated cleanup change.
2. Define and test the four Nexus core objects locally, before adding any
   remote connector.
3. Adapt Vision3 and MedVoice to write those objects.
4. Implement a manual export/import prototype with Galen using the first
   contract above.
5. Only after provenance, de-identification and review flows work, connect
   RehabScreenLab and XiaoZhu.

## Acceptance criteria for the first vertical slice

- A tester can create a synthetic case and one assessment in Nexus.
- A Vision3 or MedVoice item is stored with source and time provenance.
- A user explicitly exports a de-identified research context to Galen.
- Galen returns one source-linked evidence packet or artifact.
- Nexus can open that result and show which assessment/evidence created it.
- No identifiable patient field is written to the Galen workspace.
