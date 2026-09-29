use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    Candidate,
    Verified,
    Disputed,
    Rejected,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CollectionContext {
    NaturalStanding,
    InBrace,
    ImmediateOutOfBrace,
    OutOfBraceTimed,
    SurfaceAssessment,
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    Baseline,
    Imaging,
    Assessment,
    Intervention,
    FollowUp,
    Outcome,
    Other,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReviewStatus {
    Open,
    Resolved,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CohortStatus {
    Included,
    Excluded,
    PendingReview,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CaseRecord {
    pub case_id: String,
    pub research_id: Option<String>,
    pub demographics: Value,
    pub condition: Value,
    pub source_ids: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SourcePage {
    pub pdf_page: Option<u32>,
    pub book_page: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SourceArtifact {
    pub source_id: String,
    pub kind: String,
    pub title: String,
    pub content_hash: Option<String>,
    pub pages: Vec<SourcePage>,
    pub immutable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SourceLocator {
    pub source_id: String,
    pub pdf_page: Option<u32>,
    pub book_page: Option<u32>,
    pub channel: String,
    pub figure: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClinicalEvent {
    pub event_id: String,
    pub case_id: String,
    pub event_type: EventType,
    pub occurred_at: String,
    pub collection_context: CollectionContext,
    pub interventions: Vec<String>,
    pub source_ids: Vec<String>,
    pub verification_status: VerificationStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Observation {
    pub observation_id: String,
    pub case_id: String,
    pub event_id: String,
    pub metric: String,
    pub region: String,
    pub value: Option<Value>,
    pub unit: String,
    pub collection_context: CollectionContext,
    pub source_locator: SourceLocator,
    pub verification_status: VerificationStatus,
    pub note: Option<String>,
    #[serde(default)]
    pub source_record_id: Option<String>,
    #[serde(default)]
    pub protocol: Option<ObservationProtocolRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ObservationProtocolRef {
    pub registry_id: String,
    pub registry_version: String,
    pub original_metric: String,
    pub evidence_kind: crate::rehab_protocol::EvidenceKind,
    pub allowed_use: crate::rehab_protocol::AllowedUse,
    pub expected_unit: Option<String>,
    pub unit_matches: bool,
    pub registered: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ObservationReviewAction {
    Accept,
    Reject,
    Correct,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResearchFollowUpAction {
    #[default]
    None,
    Recapture,
    ScheduleRetest,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObservationReviewInput {
    pub case_id: String,
    pub observation_id: String,
    pub expected_revision: u32,
    pub action: ObservationReviewAction,
    pub reason: String,
    pub reviewer: String,
    pub corrected_value: Option<Value>,
    pub corrected_unit: Option<String>,
    #[serde(default)]
    pub follow_up_action: ResearchFollowUpAction,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ObservationReviewRecord {
    pub review_id: String,
    pub observation_id: String,
    pub action: ObservationReviewAction,
    pub reason: String,
    pub reviewer: String,
    pub reviewed_at: String,
    pub previous_value: Option<Value>,
    pub previous_unit: String,
    pub previous_status: VerificationStatus,
    pub resulting_value: Option<Value>,
    pub resulting_unit: String,
    pub resulting_status: VerificationStatus,
    #[serde(default)]
    pub follow_up_action: ResearchFollowUpAction,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReviewOption {
    pub option_id: String,
    pub label: String,
    pub value: Value,
    pub channel: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReviewDecision {
    pub decision_id: String,
    pub case_id: String,
    pub target_observation_id: String,
    pub question: String,
    pub options: Vec<ReviewOption>,
    pub selected_option_id: Option<String>,
    pub status: ReviewStatus,
    pub reviewer: Option<String>,
    pub reviewed_at: Option<String>,
    pub impact_scope: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CohortRow {
    pub case_id: String,
    pub revision: u32,
    pub status: CohortStatus,
    pub reasons: Vec<String>,
    pub selected_observation_ids: Vec<String>,
    pub derived_values: BTreeMap<String, Value>,
    pub source_coverage: f64,
    pub open_review_count: usize,
    pub generated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RehabCaseBundle {
    pub schema_version: u32,
    pub revision: u32,
    pub case_record: CaseRecord,
    pub sources: Vec<SourceArtifact>,
    pub events: Vec<ClinicalEvent>,
    pub observations: Vec<Observation>,
    pub review_decisions: Vec<ReviewDecision>,
    #[serde(default)]
    pub observation_reviews: Vec<ObservationReviewRecord>,
    pub cohort_row: CohortRow,
}

#[derive(Debug, Clone)]
struct NormalizedMeasurement {
    timepoint: String,
    metric: String,
    value: f64,
    unit: String,
    source_record_id: Option<String>,
    verification_status: VerificationStatus,
    protocol: ObservationProtocolRef,
    protocol_note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RehabCaseSummary {
    pub case_id: String,
    pub revision: u32,
    pub status: CohortStatus,
    pub event_count: usize,
    pub observation_count: usize,
    pub open_review_count: usize,
    pub updated_at: String,
}

/// A normalized observation flowing from the Data Quality Lab into a durable
/// Rehab ID timeline. Values are already mapped to Galen canonical metric
/// names in the UI, but the backend owns the final identifiers, provenance and
/// persistent case files.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GovernedMeasurementInput {
    pub case_id: String,
    pub timepoint: String,
    pub metric: String,
    pub value: f64,
    pub unit: String,
    #[serde(default)]
    pub source_record_id: Option<String>,
    #[serde(default)]
    pub verification_status: Option<VerificationStatus>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GovernedTimelineImportInput {
    pub dataset_path: String,
    pub quality_report_path: String,
    pub measurements: Vec<GovernedMeasurementInput>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GovernedTimelineImportOutput {
    pub case_ids: Vec<String>,
    pub imported_event_count: usize,
    pub imported_observation_count: usize,
    pub candidate_observation_count: usize,
    pub skipped_observation_count: usize,
    pub receipt: crate::artifact::ArtifactRecord,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GovernedTimelineImportReceipt {
    schema_version: u32,
    imported_at: String,
    dataset_path: String,
    quality_report_path: String,
    dataset_sha256: String,
    case_ids: Vec<String>,
    imported_event_count: usize,
    imported_observation_count: usize,
    candidate_observation_count: usize,
    skipped_observation_count: usize,
}

#[derive(Debug, Deserialize)]
struct Dataset {
    cases: Vec<DatasetCase>,
}

#[derive(Debug, Deserialize)]
struct DatasetCase {
    case_id: String,
    source_case_number: u32,
    #[serde(default)]
    source_pages: Vec<SourcePage>,
    demographics: Value,
    condition: Value,
    #[serde(default)]
    events: Vec<DatasetEvent>,
    #[serde(default)]
    observations: Vec<DatasetObservation>,
    #[serde(default)]
    source_conflicts: Vec<DatasetConflict>,
}

#[derive(Debug, Deserialize)]
struct DatasetEvent {
    event_id: String,
    date: String,
    context: String,
    #[serde(default)]
    interventions: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct DatasetObservation {
    observation_id: String,
    event_id: String,
    metric: String,
    region: String,
    value: Option<Value>,
    unit: String,
    verification_status: VerificationStatus,
    source: DatasetLocator,
    note: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DatasetLocator {
    pdf_page: Option<u32>,
    book_page: Option<u32>,
    channel: String,
    figure: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DatasetConflict {
    conflict_id: String,
    observation_id: String,
    alternatives: Vec<DatasetAlternative>,
}

#[derive(Debug, Deserialize)]
struct DatasetAlternative {
    value: Value,
    channel: String,
}

pub fn import_ais_case(
    workspace: &Path,
    source_path: &str,
    case_id: &str,
) -> Result<RehabCaseBundle, String> {
    validate_id(case_id)?;
    let source_file =
        crate::tools::workspace_path::resolve_workspace_path_from_root(workspace, source_path)?;
    let text = std::fs::read_to_string(&source_file)
        .map_err(|error| format!("读取 AIS 病例集失败: {error}"))?;
    let dataset: Dataset =
        serde_json::from_str(&text).map_err(|error| format!("AIS 病例集格式无效: {error}"))?;
    let source = dataset
        .cases
        .into_iter()
        .find(|case| case.case_id == case_id)
        .ok_or_else(|| format!("病例集内找不到 {case_id}"))?;

    let now = now_timestamp();
    let source_id = format!("ais-textbook-case-{}", source.source_case_number);
    let context_by_event: BTreeMap<_, _> = source
        .events
        .iter()
        .map(|event| (event.event_id.clone(), map_context(&event.context)))
        .collect();
    let events = source
        .events
        .into_iter()
        .map(|event| ClinicalEvent {
            event_type: map_event_type(&event.event_id),
            collection_context: map_context(&event.context),
            event_id: event.event_id,
            case_id: source.case_id.clone(),
            occurred_at: event.date,
            interventions: event.interventions,
            source_ids: vec![source_id.clone()],
            verification_status: VerificationStatus::Verified,
        })
        .collect();
    let observations = source
        .observations
        .into_iter()
        .map(|observation| Observation {
            collection_context: context_by_event
                .get(&observation.event_id)
                .copied()
                .unwrap_or(CollectionContext::Unknown),
            source_locator: SourceLocator {
                source_id: source_id.clone(),
                pdf_page: observation.source.pdf_page,
                book_page: observation.source.book_page,
                channel: observation.source.channel,
                figure: observation.source.figure,
            },
            observation_id: observation.observation_id,
            case_id: source.case_id.clone(),
            event_id: observation.event_id,
            metric: observation.metric,
            region: observation.region,
            value: observation.value,
            unit: observation.unit,
            verification_status: observation.verification_status,
            note: observation.note,
            source_record_id: None,
            protocol: None,
        })
        .collect();
    let review_decisions = source
        .source_conflicts
        .into_iter()
        .map(|conflict| ReviewDecision {
            decision_id: conflict.conflict_id,
            case_id: source.case_id.clone(),
            target_observation_id: conflict.observation_id,
            question: "来源之间存在冲突，应采用哪一个观察值？".to_string(),
            options: conflict
                .alternatives
                .into_iter()
                .enumerate()
                .map(|(index, alternative)| ReviewOption {
                    option_id: format!("option-{}", index + 1),
                    label: format!("{}：{}", alternative.channel, alternative.value),
                    value: alternative.value,
                    channel: alternative.channel,
                })
                .collect(),
            selected_option_id: None,
            status: ReviewStatus::Open,
            reviewer: None,
            reviewed_at: None,
            impact_scope: vec!["target_observation".into(), "cohort_row".into()],
        })
        .collect();

    let mut bundle = RehabCaseBundle {
        schema_version: SCHEMA_VERSION,
        revision: 1,
        case_record: CaseRecord {
            case_id: source.case_id,
            research_id: None,
            demographics: source.demographics,
            condition: source.condition,
            source_ids: vec![source_id.clone()],
            created_at: now.clone(),
            updated_at: now,
        },
        sources: vec![SourceArtifact {
            source_id,
            kind: "textbook_case".into(),
            title: format!("AIS textbook case {}", source.source_case_number),
            content_hash: None,
            pages: source.source_pages,
            immutable: true,
        }],
        events,
        observations,
        review_decisions,
        observation_reviews: Vec::new(),
        cohort_row: empty_cohort(case_id),
    };
    bundle.cohort_row = compute_cohort_row(&bundle);
    save_case_bundle(workspace, &bundle)?;
    Ok(bundle)
}

/// Persist cleaned, canonical research measurements into the same case bundles
/// that power the Rehab ID longitudinal view. The raw input is never moved or
/// overwritten; the case stores immutable source references plus a receipt.
pub fn import_governed_timeline(
    workspace: &Path,
    input: GovernedTimelineImportInput,
) -> Result<GovernedTimelineImportOutput, String> {
    const MAX_MEASUREMENTS: usize = 100_000;
    if input.measurements.is_empty() {
        return Err("没有可写入 Rehab ID 的数值观察记录。".into());
    }
    if input.measurements.len() > MAX_MEASUREMENTS {
        return Err(format!(
            "单次最多写入 {MAX_MEASUREMENTS} 条观察记录，请拆分数据集。"
        ));
    }
    let dataset_file = crate::tools::workspace_path::resolve_workspace_path_from_root(
        workspace,
        &input.dataset_path,
    )?;
    let quality_file = crate::tools::workspace_path::resolve_workspace_path_from_root(
        workspace,
        &input.quality_report_path,
    )?;
    if !dataset_file.is_file() || !quality_file.is_file() {
        return Err("清洗数据或质量报告不存在，无法建立可追溯时间轴。".into());
    }
    let dataset_bytes =
        std::fs::read(&dataset_file).map_err(|error| format!("读取清洗数据失败: {error}"))?;
    let quality_bytes =
        std::fs::read(&quality_file).map_err(|error| format!("读取质量报告失败: {error}"))?;
    let dataset_hash = format!("{:x}", Sha256::digest(&dataset_bytes));
    let quality_hash = format!("{:x}", Sha256::digest(&quality_bytes));
    let dataset_source_id = format!("governed-dataset-{}", &dataset_hash[..12]);
    let quality_source_id = format!("quality-report-{}", &quality_hash[..12]);

    let mut grouped: BTreeMap<String, Vec<NormalizedMeasurement>> = BTreeMap::new();
    for measurement in input.measurements {
        if !measurement.value.is_finite() {
            return Err("观察值包含非有限数值，未写入 Rehab ID。".into());
        }
        let case_id = normalize_governed_id(&measurement.case_id, "Rehab ID")?;
        let original_metric = normalize_governed_id(&measurement.metric, "指标")?;
        let unit = clean_unit(&measurement.unit);
        let protocol = crate::rehab_protocol::resolve_metric(&original_metric, &unit)?;
        if protocol.allowed_use == crate::rehab_protocol::AllowedUse::Excluded {
            return Err(format!(
                "指标 {original_metric} 被协议 {}@{} 排除，不能写入 Rehab ID。",
                protocol.registry_id, protocol.registry_version
            ));
        }
        let mut verification_status = measurement
            .verification_status
            .unwrap_or(VerificationStatus::Verified);
        let mut protocol_notes = Vec::new();
        if !protocol.registered {
            verification_status = VerificationStatus::Candidate;
            protocol_notes.push("指标尚未注册，只能进入研究候选队列");
        }
        if !protocol.unit_matches {
            verification_status = VerificationStatus::Candidate;
            protocol_notes.push("单位与协议不一致，必须更正或人工确认");
        }
        let timepoint = normalize_timepoint(&measurement.timepoint)?;
        grouped
            .entry(case_id)
            .or_default()
            .push(NormalizedMeasurement {
                timepoint,
                metric: protocol.canonical_metric.clone(),
                value: measurement.value,
                unit,
                source_record_id: measurement.source_record_id.map(clean_source_record_id),
                verification_status,
                protocol: ObservationProtocolRef {
                    registry_id: protocol.registry_id,
                    registry_version: protocol.registry_version,
                    original_metric,
                    evidence_kind: protocol.evidence_kind,
                    allowed_use: protocol.allowed_use,
                    expected_unit: protocol.expected_unit,
                    unit_matches: protocol.unit_matches,
                    registered: protocol.registered,
                },
                protocol_note: (!protocol_notes.is_empty()).then(|| protocol_notes.join("；")),
            });
    }

    let mut imported_event_count = 0;
    let mut imported_observation_count = 0;
    let mut candidate_observation_count = 0;
    let mut skipped_observation_count = 0;
    let mut case_ids = Vec::new();
    let import_prefix = &dataset_hash[..10];
    for (case_id, measurements) in grouped {
        let existing_path = case_path(workspace, &case_id);
        let mut bundle = if existing_path.exists() {
            // A malformed existing case must never be silently replaced by a
            // fresh one during import; that would destroy the very provenance
            // this path is meant to protect.
            load_case_bundle(workspace, &case_id)?
        } else {
            new_governed_case(&case_id)
        };
        let mut changed = false;
        if !bundle
            .sources
            .iter()
            .any(|source| source.source_id == dataset_source_id)
        {
            bundle.sources.push(SourceArtifact {
                source_id: dataset_source_id.clone(),
                kind: "governed_dataset".into(),
                title: input.dataset_path.clone(),
                content_hash: Some(dataset_hash.clone()),
                pages: Vec::new(),
                immutable: true,
            });
            bundle
                .case_record
                .source_ids
                .push(dataset_source_id.clone());
            changed = true;
        }
        if !bundle
            .sources
            .iter()
            .any(|source| source.source_id == quality_source_id)
        {
            bundle.sources.push(SourceArtifact {
                source_id: quality_source_id.clone(),
                kind: "quality_report".into(),
                title: input.quality_report_path.clone(),
                content_hash: Some(quality_hash.clone()),
                pages: Vec::new(),
                immutable: true,
            });
            bundle
                .case_record
                .source_ids
                .push(quality_source_id.clone());
            changed = true;
        }
        bundle.case_record.source_ids.sort();
        bundle.case_record.source_ids.dedup();

        let mut event_ids: BTreeMap<String, String> = BTreeMap::new();
        for measurement in &measurements {
            let event_id = event_ids
                .entry(measurement.timepoint.clone())
                .or_insert_with(|| {
                    format!(
                        "data-{import_prefix}-{}-{}",
                        slug(&measurement.timepoint, 32),
                        hash_fragment(&measurement.timepoint, 8),
                    )
                })
                .clone();
            if !bundle.events.iter().any(|event| event.event_id == event_id) {
                let event_status = if measurements
                    .iter()
                    .filter(|item| item.timepoint == measurement.timepoint)
                    .all(|item| item.verification_status == VerificationStatus::Verified)
                {
                    VerificationStatus::Verified
                } else {
                    VerificationStatus::Candidate
                };
                bundle.events.push(ClinicalEvent {
                    event_id: event_id.clone(),
                    case_id: case_id.clone(),
                    event_type: event_type_for_timepoint(&measurement.timepoint),
                    occurred_at: measurement.timepoint.clone(),
                    collection_context: CollectionContext::Unknown,
                    interventions: Vec::new(),
                    source_ids: vec![dataset_source_id.clone()],
                    verification_status: event_status,
                });
                imported_event_count += 1;
                changed = true;
            }
        }
        let mut per_event_metric: BTreeMap<(String, String), usize> = BTreeMap::new();
        for measurement in measurements {
            let event_id = event_ids
                .get(&measurement.timepoint)
                .cloned()
                .ok_or("无法为观察值创建时间点")?;
            let ordinal = per_event_metric
                .entry((event_id.clone(), measurement.metric.clone()))
                .and_modify(|value| *value += 1)
                .or_insert(1);
            let observation_id = format!(
                "obs-{import_prefix}-{event_id}-{}-{ordinal}",
                slug(&measurement.metric, 32)
            );
            if bundle
                .observations
                .iter()
                .any(|observation| observation.observation_id == observation_id)
            {
                skipped_observation_count += 1;
                continue;
            }
            let verification_status = measurement.verification_status;
            let provenance_note = measurement
                .source_record_id
                .as_deref()
                .map(|record_id| format!("来自可追溯数据版本；上游来源记录 {record_id}"))
                .unwrap_or_else(|| "来自 Galen 数据体检后的可追溯数据版本".into());
            let note = Some(match measurement.protocol_note {
                Some(protocol_note) => format!("{provenance_note}；{protocol_note}"),
                None => provenance_note,
            });
            bundle.observations.push(Observation {
                observation_id,
                case_id: case_id.clone(),
                event_id,
                metric: measurement.metric,
                region: "whole_body".into(),
                value: Some(Value::from(measurement.value)),
                unit: measurement.unit,
                collection_context: CollectionContext::Unknown,
                source_locator: SourceLocator {
                    source_id: dataset_source_id.clone(),
                    pdf_page: None,
                    book_page: None,
                    channel: "governed_dataset".into(),
                    figure: None,
                },
                verification_status,
                note,
                source_record_id: measurement.source_record_id,
                protocol: Some(measurement.protocol),
            });
            imported_observation_count += 1;
            if verification_status == VerificationStatus::Candidate {
                candidate_observation_count += 1;
            }
            changed = true;
        }
        if changed {
            bundle.revision += 1;
            bundle.case_record.updated_at = now_timestamp();
            bundle.cohort_row = compute_cohort_row(&bundle);
            save_case_bundle(workspace, &bundle)?;
        }
        case_ids.push(case_id);
    }

    let receipt_data = GovernedTimelineImportReceipt {
        schema_version: SCHEMA_VERSION,
        imported_at: now_timestamp(),
        dataset_path: input.dataset_path,
        quality_report_path: input.quality_report_path,
        dataset_sha256: dataset_hash,
        case_ids: case_ids.clone(),
        imported_event_count,
        imported_observation_count,
        candidate_observation_count,
        skipped_observation_count,
    };
    let stamp = now_millis();
    let receipt_path = format!("output/galen-rehab-import-{stamp}.json");
    std::fs::create_dir_all(workspace.join("output"))
        .map_err(|error| format!("创建导入回执目录失败: {error}"))?;
    std::fs::write(
        workspace.join(&receipt_path),
        serde_json::to_string_pretty(&receipt_data)
            .map_err(|error| format!("序列化导入回执失败: {error}"))?,
    )
    .map_err(|error| format!("写入导入回执失败: {error}"))?;
    let task_id = crate::research_task::load_active_task(workspace)?.map(|task| task.task_id);
    let receipt = crate::artifact::register_file(workspace, &receipt_path, task_id, None)?;
    Ok(GovernedTimelineImportOutput {
        case_ids,
        imported_event_count,
        imported_observation_count,
        candidate_observation_count,
        skipped_observation_count,
        receipt,
    })
}

pub fn load_case_bundle(workspace: &Path, case_id: &str) -> Result<RehabCaseBundle, String> {
    validate_id(case_id)?;
    let path = case_path(workspace, case_id);
    let text = std::fs::read_to_string(&path)
        .map_err(|error| format!("读取康复病例 {} 失败: {error}", path.display()))?;
    let mut bundle: RehabCaseBundle =
        serde_json::from_str(&text).map_err(|error| format!("康复病例数据无效: {error}"))?;
    let expected_open_reviews = pending_review_target_count(&bundle);
    if bundle.cohort_row.open_review_count != expected_open_reviews {
        bundle.revision += 1;
        bundle.case_record.updated_at = now_timestamp();
        bundle.cohort_row = compute_cohort_row(&bundle);
        save_case_bundle(workspace, &bundle)?;
    }
    Ok(bundle)
}

pub fn list_case_summaries(workspace: &Path) -> Result<Vec<RehabCaseSummary>, String> {
    let directory = cases_dir(workspace);
    if !directory.exists() {
        return Ok(Vec::new());
    }
    let mut summaries = Vec::new();
    for entry in
        std::fs::read_dir(&directory).map_err(|error| format!("读取康复病例目录失败: {error}"))?
    {
        let path = entry
            .map_err(|error| format!("读取康复病例项失败: {error}"))?
            .path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let text = std::fs::read_to_string(&path)
            .map_err(|error| format!("读取康复病例 {} 失败: {error}", path.display()))?;
        let bundle: RehabCaseBundle = serde_json::from_str(&text)
            .map_err(|error| format!("康复病例 {} 数据无效: {error}", path.display()))?;
        summaries.push(RehabCaseSummary {
            case_id: bundle.case_record.case_id,
            revision: bundle.revision,
            status: bundle.cohort_row.status,
            event_count: bundle.events.len(),
            observation_count: bundle.observations.len(),
            open_review_count: bundle.cohort_row.open_review_count,
            updated_at: bundle.case_record.updated_at,
        });
    }
    summaries.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    Ok(summaries)
}

pub fn resolve_review(
    workspace: &Path,
    case_id: &str,
    decision_id: &str,
    option_id: &str,
    reviewer: &str,
) -> Result<RehabCaseBundle, String> {
    let mut bundle = load_case_bundle(workspace, case_id)?;
    let decision = bundle
        .review_decisions
        .iter_mut()
        .find(|decision| decision.decision_id == decision_id)
        .ok_or_else(|| format!("找不到裁决 {decision_id}"))?;
    let option = decision
        .options
        .iter()
        .find(|option| option.option_id == option_id)
        .ok_or_else(|| format!("找不到裁决选项 {option_id}"))?;
    let observation = bundle
        .observations
        .iter_mut()
        .find(|observation| observation.observation_id == decision.target_observation_id)
        .ok_or_else(|| "裁决所指向的观察值不存在".to_string())?;

    observation.value = Some(option.value.clone());
    observation.verification_status = VerificationStatus::Verified;
    observation.source_locator.channel = option.channel.clone();
    decision.selected_option_id = Some(option.option_id.clone());
    decision.status = ReviewStatus::Resolved;
    decision.reviewer = Some(clean_reviewer(reviewer));
    decision.reviewed_at = Some(now_timestamp());
    bundle.revision += 1;
    bundle.case_record.updated_at = now_timestamp();
    bundle.cohort_row = compute_cohort_row(&bundle);
    save_case_bundle(workspace, &bundle)?;
    Ok(bundle)
}

pub fn review_observation(
    workspace: &Path,
    input: ObservationReviewInput,
) -> Result<RehabCaseBundle, String> {
    validate_id(&input.case_id)?;
    if input.reason.trim().is_empty() {
        return Err("观察审核必须填写具体理由。".into());
    }
    if input.reviewer.trim().is_empty() {
        return Err("观察审核必须记录审核人。".into());
    }
    let mut bundle = load_case_bundle(workspace, &input.case_id)?;
    if bundle.revision != input.expected_revision {
        return Err(format!(
            "REHAB_CASE_CONFLICT: 当前 revision={}，请求 revision={}",
            bundle.revision, input.expected_revision
        ));
    }
    let observation_index = bundle
        .observations
        .iter()
        .position(|item| item.observation_id == input.observation_id)
        .ok_or_else(|| format!("找不到观察值 {}", input.observation_id))?;
    let observation = &mut bundle.observations[observation_index];
    let previous_value = observation.value.clone();
    let previous_unit = observation.unit.clone();
    let previous_status = observation.verification_status;

    match input.action {
        ObservationReviewAction::Accept => {
            if observation.protocol.as_ref().is_some_and(|protocol| {
                protocol.allowed_use == crate::rehab_protocol::AllowedUse::Excluded
                    || !protocol.unit_matches
            }) {
                return Err("协议排除或单位不匹配的观察不能直接接受，请更正或拒绝。".into());
            }
            observation.verification_status = VerificationStatus::Verified;
        }
        ObservationReviewAction::Reject => {
            observation.verification_status = VerificationStatus::Rejected;
        }
        ObservationReviewAction::Correct => {
            let corrected_value = input
                .corrected_value
                .clone()
                .ok_or("更正观察必须提供 correctedValue。")?;
            if corrected_value.as_f64().is_none() && corrected_value.as_str().is_none() {
                return Err("correctedValue 只支持有限数值或文本。".into());
            }
            if corrected_value
                .as_f64()
                .is_some_and(|value| !value.is_finite())
            {
                return Err("correctedValue 不能是非有限数值。".into());
            }
            let corrected_unit = input
                .corrected_unit
                .as_deref()
                .map(clean_unit)
                .unwrap_or_else(|| observation.unit.clone());
            let original_metric = observation
                .protocol
                .as_ref()
                .map(|protocol| protocol.original_metric.clone())
                .unwrap_or_else(|| observation.metric.clone());
            let resolution =
                crate::rehab_protocol::resolve_metric(&original_metric, &corrected_unit)?;
            if resolution.allowed_use == crate::rehab_protocol::AllowedUse::Excluded {
                return Err("被协议排除的指标不能通过人工更正进入事实层。".into());
            }
            if !resolution.unit_matches {
                return Err(format!(
                    "更正单位仍与协议不一致；预期 {}。",
                    resolution
                        .expected_unit
                        .as_deref()
                        .unwrap_or("协议允许单位")
                ));
            }
            observation.value = Some(corrected_value);
            observation.unit = corrected_unit;
            observation.metric = resolution.canonical_metric.clone();
            observation.protocol = Some(ObservationProtocolRef {
                registry_id: resolution.registry_id,
                registry_version: resolution.registry_version,
                original_metric,
                evidence_kind: resolution.evidence_kind,
                allowed_use: resolution.allowed_use,
                expected_unit: resolution.expected_unit,
                unit_matches: resolution.unit_matches,
                registered: resolution.registered,
            });
            observation.verification_status = VerificationStatus::Verified;
        }
    }

    let resulting_value = observation.value.clone();
    let resulting_unit = observation.unit.clone();
    let resulting_status = observation.verification_status;
    let event_id = observation.event_id.clone();
    bundle.observation_reviews.push(ObservationReviewRecord {
        review_id: format!(
            "review-{}-{}",
            slug(&input.observation_id, 36),
            now_millis()
        ),
        observation_id: input.observation_id,
        action: input.action,
        reason: input.reason.trim().chars().take(500).collect(),
        reviewer: clean_reviewer(&input.reviewer),
        reviewed_at: now_timestamp(),
        previous_value,
        previous_unit,
        previous_status,
        resulting_value,
        resulting_unit,
        resulting_status,
        follow_up_action: input.follow_up_action,
    });
    refresh_event_verification(&mut bundle, &event_id);
    bundle.revision += 1;
    bundle.case_record.updated_at = now_timestamp();
    bundle.cohort_row = compute_cohort_row(&bundle);
    save_case_bundle(workspace, &bundle)?;
    Ok(bundle)
}

fn refresh_event_verification(bundle: &mut RehabCaseBundle, event_id: &str) {
    let statuses = bundle
        .observations
        .iter()
        .filter(|item| item.event_id == event_id)
        .map(|item| item.verification_status)
        .collect::<Vec<_>>();
    let status = if !statuses.is_empty()
        && statuses
            .iter()
            .all(|status| *status == VerificationStatus::Verified)
    {
        VerificationStatus::Verified
    } else if !statuses.is_empty()
        && statuses
            .iter()
            .all(|status| *status == VerificationStatus::Rejected)
    {
        VerificationStatus::Rejected
    } else {
        VerificationStatus::Candidate
    };
    if let Some(event) = bundle
        .events
        .iter_mut()
        .find(|event| event.event_id == event_id)
    {
        event.verification_status = status;
    }
}

pub fn compute_cohort_row(bundle: &RehabCaseBundle) -> CohortRow {
    let verified: Vec<_> = bundle
        .observations
        .iter()
        .filter(|observation| observation.verification_status == VerificationStatus::Verified)
        .collect();
    let mut derived_values = BTreeMap::new();
    let mut selected_observation_ids = Vec::new();

    for baseline in verified.iter().filter(|observation| {
        bundle.events.iter().any(|event| {
            event.event_id == observation.event_id && event.event_type == EventType::Baseline
        })
    }) {
        let Some(baseline_value) = baseline.value.as_ref().and_then(Value::as_f64) else {
            continue;
        };
        let follow_up = verified.iter().find(|observation| {
            bundle.events.iter().any(|event| {
                event.event_id == observation.event_id && event.event_type == EventType::FollowUp
            }) && observation.metric == baseline.metric
                && observation.region == baseline.region
                && observation.value.as_ref().and_then(Value::as_f64).is_some()
        });
        let Some(follow_up) = follow_up else { continue };
        let follow_up_value = follow_up
            .value
            .as_ref()
            .and_then(Value::as_f64)
            .unwrap_or_default();
        let key = format!("{}_{}", baseline.metric, baseline.region);
        derived_values.insert(format!("{key}_baseline"), Value::from(baseline_value));
        derived_values.insert(format!("{key}_follow_up"), Value::from(follow_up_value));
        derived_values.insert(
            format!("{key}_change"),
            Value::from(follow_up_value - baseline_value),
        );
        selected_observation_ids.push(baseline.observation_id.clone());
        selected_observation_ids.push(follow_up.observation_id.clone());
    }
    selected_observation_ids.sort();
    selected_observation_ids.dedup();

    let open_review_count = pending_review_target_count(bundle);
    let located = verified
        .iter()
        .filter(|observation| {
            observation
                .source_locator
                .source_id
                .starts_with("governed-dataset-")
                || observation.source_locator.pdf_page.is_some()
                || observation.source_locator.book_page.is_some()
                || observation.source_locator.figure.is_some()
        })
        .count();
    let source_coverage = if verified.is_empty() {
        0.0
    } else {
        located as f64 / verified.len() as f64
    };
    let mut reasons = Vec::new();
    let status = if derived_values.is_empty() {
        reasons.push("缺少可配对的已核验基线与随访观察值".into());
        CohortStatus::PendingReview
    } else {
        reasons.push("已从可追溯且已核验的纵向观察值生成".into());
        if open_review_count > 0 {
            reasons.push("仍有不影响当前纵向主结局的开放裁决".into());
        }
        CohortStatus::Included
    };

    CohortRow {
        case_id: bundle.case_record.case_id.clone(),
        revision: bundle.revision,
        status,
        reasons,
        selected_observation_ids,
        derived_values,
        source_coverage,
        open_review_count,
        generated_at: now_timestamp(),
    }
}

fn pending_review_target_count(bundle: &RehabCaseBundle) -> usize {
    let mut open_review_targets = bundle
        .observations
        .iter()
        .filter(|observation| {
            matches!(
                observation.verification_status,
                VerificationStatus::Candidate | VerificationStatus::Disputed
            )
        })
        .map(|observation| observation.observation_id.clone())
        .collect::<BTreeSet<_>>();
    open_review_targets.extend(
        bundle
            .review_decisions
            .iter()
            .filter(|decision| decision.status == ReviewStatus::Open)
            .map(|decision| decision.target_observation_id.clone()),
    );
    open_review_targets.len()
}

fn save_case_bundle(workspace: &Path, bundle: &RehabCaseBundle) -> Result<(), String> {
    validate_id(&bundle.case_record.case_id)?;
    let path = case_path(workspace, &bundle.case_record.case_id);
    let json = serde_json::to_string_pretty(bundle)
        .map_err(|error| format!("序列化康复病例失败: {error}"))?;
    write_json(&path, &json)
}

fn write_json(path: &Path, content: &str) -> Result<(), String> {
    let parent = path.parent().ok_or("康复病例路径没有父目录")?;
    std::fs::create_dir_all(parent).map_err(|error| format!("创建康复病例目录失败: {error}"))?;
    let pending = path.with_extension("json.pending");
    std::fs::write(&pending, content)
        .map_err(|error| format!("写入康复病例临时文件失败: {error}"))?;
    if path.exists() {
        let backup = path.with_extension("json.backup");
        let _ = std::fs::remove_file(&backup);
        std::fs::rename(path, &backup).map_err(|error| format!("备份旧康复病例失败: {error}"))?;
        if let Err(error) = std::fs::rename(&pending, path) {
            let _ = std::fs::rename(&backup, path);
            return Err(format!("替换康复病例失败: {error}"));
        }
        let _ = std::fs::remove_file(backup);
    } else {
        std::fs::rename(&pending, path).map_err(|error| format!("保存康复病例失败: {error}"))?;
    }
    Ok(())
}

fn cases_dir(workspace: &Path) -> PathBuf {
    workspace.join(".galen").join("rehab-context").join("cases")
}

fn case_path(workspace: &Path, case_id: &str) -> PathBuf {
    cases_dir(workspace).join(format!("{case_id}.json"))
}

fn validate_id(value: &str) -> Result<(), String> {
    if value.is_empty()
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        })
    {
        return Err("康复病例 ID 无效".into());
    }
    Ok(())
}

fn map_context(value: &str) -> CollectionContext {
    match value {
        "out_of_brace" => CollectionContext::NaturalStanding,
        "in_brace" => CollectionContext::InBrace,
        "out_of_brace_24h" => CollectionContext::OutOfBraceTimed,
        "clinical_surface_exam" => CollectionContext::SurfaceAssessment,
        value if value.contains("surface") => CollectionContext::SurfaceAssessment,
        value if value.contains("out_of_brace") => CollectionContext::OutOfBraceTimed,
        _ => CollectionContext::Unknown,
    }
}

fn map_event_type(event_id: &str) -> EventType {
    match event_id {
        "baseline" => EventType::Baseline,
        "in_brace" => EventType::Intervention,
        "follow_up" => EventType::FollowUp,
        _ => EventType::Other,
    }
}

fn empty_cohort(case_id: &str) -> CohortRow {
    CohortRow {
        case_id: case_id.to_string(),
        revision: 1,
        status: CohortStatus::PendingReview,
        reasons: Vec::new(),
        selected_observation_ids: Vec::new(),
        derived_values: BTreeMap::new(),
        source_coverage: 0.0,
        open_review_count: 0,
        generated_at: now_timestamp(),
    }
}

fn new_governed_case(case_id: &str) -> RehabCaseBundle {
    let now = now_timestamp();
    RehabCaseBundle {
        schema_version: SCHEMA_VERSION,
        revision: 0,
        case_record: CaseRecord {
            case_id: case_id.to_string(),
            research_id: None,
            demographics: Value::Object(Default::default()),
            condition: serde_json::json!({"source": "governed_research_dataset"}),
            source_ids: Vec::new(),
            created_at: now.clone(),
            updated_at: now,
        },
        sources: Vec::new(),
        events: Vec::new(),
        observations: Vec::new(),
        review_decisions: Vec::new(),
        observation_reviews: Vec::new(),
        cohort_row: empty_cohort(case_id),
    }
}

fn normalize_governed_id(value: &str, label: &str) -> Result<String, String> {
    let normalized = value
        .trim()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    validate_id(&normalized)
        .map_err(|_| format!("{label} 不能为空，且只能使用字母、数字、- 或 _。"))?;
    Ok(normalized.chars().take(80).collect())
}

fn normalize_timepoint(value: &str) -> Result<String, String> {
    let cleaned = value.trim().chars().take(80).collect::<String>();
    if cleaned.is_empty() {
        Err("时间点不能为空。".into())
    } else {
        Ok(cleaned)
    }
}

fn clean_unit(value: &str) -> String {
    let unit = value.trim().chars().take(32).collect::<String>();
    if unit.is_empty() {
        "value".into()
    } else {
        unit
    }
}

fn clean_source_record_id(value: String) -> String {
    value
        .trim()
        .chars()
        .filter(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | ':')
        })
        .take(120)
        .collect()
}

fn slug(value: &str, max: usize) -> String {
    let value = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    let value = value
        .trim_matches('-')
        .chars()
        .take(max)
        .collect::<String>();
    if value.is_empty() {
        "record".into()
    } else {
        value
    }
}

fn hash_fragment(value: &str, length: usize) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
        .chars()
        .take(length)
        .collect()
}

fn event_type_for_timepoint(timepoint: &str) -> EventType {
    let value = timepoint
        .to_ascii_lowercase()
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    if value.contains("baseline") || timepoint.contains("基线") || value == "t0" {
        EventType::Baseline
    } else if value.contains("follow")
        || timepoint.contains("随访")
        || value.contains("24h")
        || value.contains("48h")
        || value.contains("week")
        || timepoint.contains("天")
    {
        EventType::FollowUp
    } else {
        EventType::Assessment
    }
}

fn clean_reviewer(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        "human-reviewer".into()
    } else {
        trimmed.chars().take(80).collect()
    }
}

fn now_timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_workspace() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("galen-rehab-context-{nonce}"));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn c025_dataset() -> &'static str {
        r#"{"cases":[{"case_id":"AIS-C025","source_case_number":25,"source_pages":[{"pdf_page":84,"book_page":71}],"demographics":{"sex":"female","age_years_at_baseline":11},"condition":{"etiology":"idiopathic_scoliosis","risser_grade":0},"events":[{"event_id":"baseline","date":"2019-05","context":"out_of_brace"},{"event_id":"in_brace","date":"2019-05","context":"in_brace"},{"event_id":"follow_up","date":"2020-12","context":"out_of_brace"}],"observations":[{"observation_id":"C025-B-T","event_id":"baseline","metric":"cobb_deg","region":"thoracic","value":45,"unit":"deg","verification_status":"verified","source":{"pdf_page":84,"book_page":71,"channel":"text"}},{"observation_id":"C025-I-T","event_id":"in_brace","metric":"cobb_deg","region":"thoracic","value":null,"unit":"deg","verification_status":"disputed","source":{"pdf_page":84,"book_page":71,"channel":"text_and_figure"}},{"observation_id":"C025-F-T","event_id":"follow_up","metric":"cobb_deg","region":"thoracic","value":26,"unit":"deg","verification_status":"verified","source":{"pdf_page":85,"book_page":72,"channel":"text"}}],"source_conflicts":[{"conflict_id":"C025-CONFLICT-INBRACE-T","observation_id":"C025-I-T","alternatives":[{"value":7,"channel":"text"},{"value":-6,"channel":"figure_annotation"}]}]}]}"#
    }

    #[test]
    fn disputed_observation_never_enters_hard_gate() {
        let workspace = fixture_workspace();
        std::fs::write(workspace.join("cases.json"), c025_dataset()).unwrap();
        let bundle = import_ais_case(&workspace, "cases.json", "AIS-C025").unwrap();
        assert_eq!(bundle.cohort_row.status, CohortStatus::Included);
        assert_eq!(bundle.cohort_row.open_review_count, 1);
        assert_eq!(
            bundle.cohort_row.derived_values["cobb_deg_thoracic_change"],
            Value::from(-19.0)
        );
        assert!(!bundle
            .cohort_row
            .selected_observation_ids
            .contains(&"C025-I-T".to_string()));
    }

    #[test]
    fn human_resolution_is_persisted_and_recomputed() {
        let workspace = fixture_workspace();
        std::fs::write(workspace.join("cases.json"), c025_dataset()).unwrap();
        import_ais_case(&workspace, "cases.json", "AIS-C025").unwrap();
        let bundle = resolve_review(
            &workspace,
            "AIS-C025",
            "C025-CONFLICT-INBRACE-T",
            "option-1",
            "reviewer-a",
        )
        .unwrap();
        assert_eq!(bundle.revision, 2);
        assert_eq!(bundle.cohort_row.open_review_count, 0);
        let observation = bundle
            .observations
            .iter()
            .find(|item| item.observation_id == "C025-I-T")
            .unwrap();
        assert_eq!(observation.value, Some(Value::from(7)));
        assert_eq!(
            observation.verification_status,
            VerificationStatus::Verified
        );
        assert_eq!(load_case_bundle(&workspace, "AIS-C025").unwrap(), bundle);
    }

    #[test]
    fn rejects_path_traversal_case_id() {
        let workspace = fixture_workspace();
        assert!(load_case_bundle(&workspace, "../escape").is_err());
    }

    #[test]
    fn governed_dataset_becomes_a_durable_rehab_id_timeline() {
        let workspace = fixture_workspace();
        std::fs::create_dir_all(workspace.join("output")).unwrap();
        std::fs::write(
            workspace.join("output/cleaned-fatigue.csv"),
            "rehab_id,timepoint,hrv_rmssd_ms,cmj_height_cm\nP-001,baseline,42,31\nP-001,24h,55,34\n",
        )
        .unwrap();
        std::fs::write(
            workspace.join("output/quality.json"),
            "{\"schemaVersion\":1}",
        )
        .unwrap();
        let input = GovernedTimelineImportInput {
            dataset_path: "output/cleaned-fatigue.csv".into(),
            quality_report_path: "output/quality.json".into(),
            measurements: vec![
                GovernedMeasurementInput {
                    case_id: "P-001".into(),
                    timepoint: "baseline".into(),
                    metric: "hrv_rmssd_ms".into(),
                    value: 42.0,
                    unit: "ms".into(),
                    source_record_id: None,
                    verification_status: None,
                },
                GovernedMeasurementInput {
                    case_id: "P-001".into(),
                    timepoint: "baseline".into(),
                    metric: "cmj_height_cm".into(),
                    value: 31.0,
                    unit: "cm".into(),
                    source_record_id: None,
                    verification_status: None,
                },
                GovernedMeasurementInput {
                    case_id: "P-001".into(),
                    timepoint: "24h".into(),
                    metric: "hrv_rmssd_ms".into(),
                    value: 55.0,
                    unit: "ms".into(),
                    source_record_id: None,
                    verification_status: None,
                },
                GovernedMeasurementInput {
                    case_id: "P-001".into(),
                    timepoint: "24h".into(),
                    metric: "cmj_height_cm".into(),
                    value: 34.0,
                    unit: "cm".into(),
                    source_record_id: None,
                    verification_status: None,
                },
            ],
        };
        let output = import_governed_timeline(&workspace, input.clone()).unwrap();
        assert_eq!(output.case_ids, vec!["P-001"]);
        assert_eq!(output.imported_event_count, 2);
        assert_eq!(output.imported_observation_count, 4);
        assert!(workspace.join(&output.receipt.path).is_file());
        let bundle = load_case_bundle(&workspace, "P-001").unwrap();
        assert_eq!(bundle.events.len(), 2);
        assert_eq!(bundle.observations.len(), 4);
        assert_eq!(bundle.sources.len(), 2);
        assert_eq!(bundle.cohort_row.status, CohortStatus::Included);
        assert_eq!(bundle.cohort_row.source_coverage, 1.0);

        let repeated = import_governed_timeline(&workspace, input).unwrap();
        assert_eq!(repeated.imported_event_count, 0);
        assert_eq!(repeated.imported_observation_count, 0);
        assert_eq!(repeated.skipped_observation_count, 4);
        assert_eq!(
            load_case_bundle(&workspace, "P-001")
                .unwrap()
                .observations
                .len(),
            4
        );
    }

    #[test]
    fn chinese_timepoints_are_never_collapsed_into_one_event() {
        let workspace = fixture_workspace();
        std::fs::create_dir_all(workspace.join("output")).unwrap();
        std::fs::write(workspace.join("output/cleaned.csv"), "metric\n1\n").unwrap();
        std::fs::write(workspace.join("output/quality.json"), "{}").unwrap();
        let output = import_governed_timeline(
            &workspace,
            GovernedTimelineImportInput {
                dataset_path: "output/cleaned.csv".into(),
                quality_report_path: "output/quality.json".into(),
                measurements: vec![
                    GovernedMeasurementInput {
                        case_id: "P-002".into(),
                        timepoint: "基线".into(),
                        metric: "hrv_rmssd_ms".into(),
                        value: 40.0,
                        unit: "ms".into(),
                        source_record_id: None,
                        verification_status: None,
                    },
                    GovernedMeasurementInput {
                        case_id: "P-002".into(),
                        timepoint: "训练后24小时".into(),
                        metric: "hrv_rmssd_ms".into(),
                        value: 38.0,
                        unit: "ms".into(),
                        source_record_id: None,
                        verification_status: None,
                    },
                    GovernedMeasurementInput {
                        case_id: "P-002".into(),
                        timepoint: "训练后48小时".into(),
                        metric: "hrv_rmssd_ms".into(),
                        value: 46.0,
                        unit: "ms".into(),
                        source_record_id: None,
                        verification_status: None,
                    },
                ],
            },
        )
        .unwrap();
        assert_eq!(output.imported_event_count, 3);
        let bundle = load_case_bundle(&workspace, "P-002").unwrap();
        assert_eq!(bundle.events.len(), 3);
        assert_ne!(bundle.events[1].event_id, bundle.events[2].event_id);
    }

    #[test]
    fn governed_import_never_replaces_a_corrupt_existing_case() {
        let workspace = fixture_workspace();
        std::fs::create_dir_all(workspace.join("output")).unwrap();
        std::fs::write(workspace.join("output/cleaned.csv"), "metric\n1\n").unwrap();
        std::fs::write(workspace.join("output/quality.json"), "{}").unwrap();
        std::fs::create_dir_all(cases_dir(&workspace)).unwrap();
        let corrupt = case_path(&workspace, "P-003");
        std::fs::write(&corrupt, "not valid json").unwrap();
        let result = import_governed_timeline(
            &workspace,
            GovernedTimelineImportInput {
                dataset_path: "output/cleaned.csv".into(),
                quality_report_path: "output/quality.json".into(),
                measurements: vec![GovernedMeasurementInput {
                    case_id: "P-003".into(),
                    timepoint: "baseline".into(),
                    metric: "rpe".into(),
                    value: 12.0,
                    unit: "score".into(),
                    source_record_id: None,
                    verification_status: None,
                }],
            },
        );
        assert!(result.is_err());
        assert_eq!(std::fs::read_to_string(corrupt).unwrap(), "not valid json");
    }

    #[test]
    fn protocol_registry_blocks_excluded_measurements() {
        let workspace = fixture_workspace();
        std::fs::create_dir_all(workspace.join("output")).unwrap();
        std::fs::write(workspace.join("output/cleaned.csv"), "metric\n1\n").unwrap();
        std::fs::write(workspace.join("output/quality.json"), "{}").unwrap();
        let error = import_governed_timeline(
            &workspace,
            GovernedTimelineImportInput {
                dataset_path: "output/cleaned.csv".into(),
                quality_report_path: "output/quality.json".into(),
                measurements: vec![GovernedMeasurementInput {
                    case_id: "RID-BLOCKED".into(),
                    timepoint: "baseline".into(),
                    metric: "adams_cobbAngleEstimate".into(),
                    value: 18.0,
                    unit: "deg".into(),
                    source_record_id: Some("assessment-x".into()),
                    verification_status: Some(VerificationStatus::Candidate),
                }],
            },
        )
        .unwrap_err();
        assert!(error.contains("协议") && error.contains("排除"));
        assert!(!case_path(&workspace, "RID-BLOCKED").exists());
    }

    #[test]
    fn observation_review_is_revision_safe_and_auditable() {
        let workspace = fixture_workspace();
        std::fs::create_dir_all(workspace.join("output")).unwrap();
        std::fs::write(workspace.join("output/cleaned.csv"), "metric\n1\n").unwrap();
        std::fs::write(workspace.join("output/quality.json"), "{}").unwrap();
        let imported = import_governed_timeline(
            &workspace,
            GovernedTimelineImportInput {
                dataset_path: "output/cleaned.csv".into(),
                quality_report_path: "output/quality.json".into(),
                measurements: vec![
                    GovernedMeasurementInput {
                        case_id: "RID-REVIEW".into(),
                        timepoint: "baseline".into(),
                        metric: "adams_atrDegrees".into(),
                        value: 7.0,
                        unit: "deg".into(),
                        source_record_id: Some("assessment-a".into()),
                        verification_status: Some(VerificationStatus::Candidate),
                    },
                    GovernedMeasurementInput {
                        case_id: "RID-REVIEW".into(),
                        timepoint: "baseline".into(),
                        metric: "rom_knee_flexion_left".into(),
                        value: 32.0,
                        unit: "cm".into(),
                        source_record_id: Some("assessment-b".into()),
                        verification_status: Some(VerificationStatus::Candidate),
                    },
                    GovernedMeasurementInput {
                        case_id: "RID-REVIEW".into(),
                        timepoint: "baseline".into(),
                        metric: "novel_sensor_index".into(),
                        value: 0.8,
                        unit: "value".into(),
                        source_record_id: Some("assessment-c".into()),
                        verification_status: Some(VerificationStatus::Candidate),
                    },
                ],
            },
        )
        .unwrap();
        let initial = load_case_bundle(&workspace, "RID-REVIEW").unwrap();
        assert_eq!(initial.cohort_row.open_review_count, 3);
        let atr_id = initial
            .observations
            .iter()
            .find(|item| item.metric == "adams_atr_deg")
            .unwrap()
            .observation_id
            .clone();
        let accepted = review_observation(
            &workspace,
            ObservationReviewInput {
                case_id: "RID-REVIEW".into(),
                observation_id: atr_id.clone(),
                expected_revision: initial.revision,
                action: ObservationReviewAction::Accept,
                reason: "已核对原始测角仪记录与单位".into(),
                reviewer: "reviewer-1".into(),
                corrected_value: None,
                corrected_unit: None,
                follow_up_action: ResearchFollowUpAction::None,
            },
        )
        .unwrap();
        assert_eq!(accepted.cohort_row.open_review_count, 2);
        assert_eq!(accepted.observation_reviews.len(), 1);
        assert_eq!(
            accepted.observation_reviews[0].previous_status,
            VerificationStatus::Candidate
        );
        let stale = review_observation(
            &workspace,
            ObservationReviewInput {
                case_id: "RID-REVIEW".into(),
                observation_id: atr_id,
                expected_revision: initial.revision,
                action: ObservationReviewAction::Reject,
                reason: "模拟过期操作".into(),
                reviewer: "reviewer-2".into(),
                corrected_value: None,
                corrected_unit: None,
                follow_up_action: ResearchFollowUpAction::None,
            },
        )
        .unwrap_err();
        assert!(stale.contains("REHAB_CASE_CONFLICT"));

        let rom_id = accepted
            .observations
            .iter()
            .find(|item| item.metric.starts_with("rom_"))
            .unwrap()
            .observation_id
            .clone();
        let corrected = review_observation(
            &workspace,
            ObservationReviewInput {
                case_id: "RID-REVIEW".into(),
                observation_id: rom_id,
                expected_revision: accepted.revision,
                action: ObservationReviewAction::Correct,
                reason: "原导出把角度单位错误标成 cm".into(),
                reviewer: "reviewer-1".into(),
                corrected_value: Some(Value::from(32.0)),
                corrected_unit: Some("deg".into()),
                follow_up_action: ResearchFollowUpAction::None,
            },
        )
        .unwrap();
        let unknown_id = corrected
            .observations
            .iter()
            .find(|item| item.metric == "novel_sensor_index")
            .unwrap()
            .observation_id
            .clone();
        let rejected = review_observation(
            &workspace,
            ObservationReviewInput {
                case_id: "RID-REVIEW".into(),
                observation_id: unknown_id,
                expected_revision: corrected.revision,
                action: ObservationReviewAction::Reject,
                reason: "设备字段尚未注册且无法追溯校准".into(),
                reviewer: "reviewer-1".into(),
                corrected_value: None,
                corrected_unit: None,
                follow_up_action: ResearchFollowUpAction::Recapture,
            },
        )
        .unwrap();
        assert_eq!(rejected.cohort_row.open_review_count, 0);
        assert_eq!(rejected.observation_reviews.len(), 3);
        assert_eq!(imported.case_ids, vec!["RID-REVIEW"]);
    }
}
