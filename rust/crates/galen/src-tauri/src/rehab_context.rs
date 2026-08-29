use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    Candidate,
    Verified,
    Disputed,
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
    pub cohort_row: CohortRow,
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
        cohort_row: empty_cohort(case_id),
    };
    bundle.cohort_row = compute_cohort_row(&bundle);
    save_case_bundle(workspace, &bundle)?;
    Ok(bundle)
}

pub fn load_case_bundle(workspace: &Path, case_id: &str) -> Result<RehabCaseBundle, String> {
    validate_id(case_id)?;
    let path = case_path(workspace, case_id);
    let text = std::fs::read_to_string(&path)
        .map_err(|error| format!("读取康复病例 {} 失败: {error}", path.display()))?;
    serde_json::from_str(&text).map_err(|error| format!("康复病例数据无效: {error}"))
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

pub fn compute_cohort_row(bundle: &RehabCaseBundle) -> CohortRow {
    let verified: Vec<_> = bundle
        .observations
        .iter()
        .filter(|observation| observation.verification_status == VerificationStatus::Verified)
        .collect();
    let mut derived_values = BTreeMap::new();
    let mut selected_observation_ids = Vec::new();

    for baseline in verified
        .iter()
        .filter(|observation| observation.event_id == "baseline")
    {
        let Some(baseline_value) = baseline.value.as_ref().and_then(Value::as_f64) else {
            continue;
        };
        let follow_up = verified.iter().find(|observation| {
            observation.event_id == "follow_up"
                && observation.metric == baseline.metric
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

    let open_review_count = bundle
        .review_decisions
        .iter()
        .filter(|decision| decision.status == ReviewStatus::Open)
        .count();
    let located = verified
        .iter()
        .filter(|observation| {
            observation.source_locator.pdf_page.is_some()
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
}
