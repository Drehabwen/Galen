use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

const REHAB_WORKBENCH_ID: &str = "rehab-workbench";
const REHABGPT_ID: &str = "rehabgpt";

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorCasePreview {
    pub case_id: String,
    pub display_name: String,
    pub session_count: usize,
    pub assessment_count: usize,
    pub timepoint_count: usize,
    pub measurement_count: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorPreview {
    pub source_id: String,
    pub source_label: String,
    pub export_path: String,
    pub exported_at: u64,
    pub connection_mode: String,
    pub patient_count: usize,
    pub session_count: usize,
    pub assessment_count: usize,
    pub timepoint_count: usize,
    pub measurement_count: usize,
    pub cases: Vec<ConnectorCasePreview>,
    pub can_import: bool,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorImportRequest {
    pub source_id: String,
    pub export_path: String,
    #[serde(default)]
    pub case_ids: Vec<String>,
    pub latest_assessments: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RehabBackup {
    version: String,
    exported_at: u64,
    #[serde(default)]
    patients: Vec<Value>,
    #[serde(default)]
    sessions: Vec<Value>,
    #[serde(default)]
    assessments: Vec<Value>,
}

#[derive(Debug, Clone)]
struct ConnectorMeasurement {
    case_id: String,
    timepoint: String,
    metric: String,
    value: f64,
    unit: String,
    assessment_id: String,
    created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorFeedbackItem {
    pub feedback_id: String,
    pub rehab_id: String,
    pub source_assessment_id: String,
    pub observation_id: String,
    pub metric: String,
    pub action: crate::rehab_context::ObservationReviewAction,
    pub follow_up_action: crate::rehab_context::ResearchFollowUpAction,
    pub reason: String,
    pub reviewer: String,
    pub reviewed_at: String,
    pub previous_value: Option<Value>,
    pub previous_unit: String,
    pub resulting_value: Option<Value>,
    pub resulting_unit: String,
    pub resulting_status: crate::rehab_context::VerificationStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorFeedbackFile {
    pub schema_version: u32,
    pub connector_id: String,
    pub generated_at: u64,
    #[serde(default)]
    pub items: Vec<ConnectorFeedbackItem>,
}

pub fn discover_latest(
    source_id: &str,
    case_hint: Option<&str>,
) -> Result<ConnectorPreview, String> {
    if source_id == REHABGPT_ID {
        let bridge = rehabgpt_bridge_snapshot_path()?;
        if !bridge.is_file() {
            return Err(
                "尚未发现 RehabGPT Connector Bridge。请先由 RehabGPT 写出最新的脱敏数据快照。"
                    .into(),
            );
        }
        return preview_export(&bridge, case_hint, "live_bridge", REHABGPT_ID, "RehabGPT");
    }
    if source_id != REHAB_WORKBENCH_ID {
        return Err(format!("尚未实现数据源：{source_id}"));
    }
    let bridge = galen_bridge_snapshot_path()?;
    if bridge.is_file() {
        return preview_export(
            &bridge,
            case_hint,
            "live_bridge",
            REHAB_WORKBENCH_ID,
            "康复师工作台",
        );
    }
    let downloads = dirs::download_dir().ok_or("无法定位系统下载目录。")?;
    let export = latest_rehab_export(&downloads)?.ok_or(
        "尚未发现康复师工作台导出。请在工作台的「系统设置 → 数据备份」中导出一次完整数据。",
    )?;
    preview_export(
        &export,
        case_hint,
        "file_fallback",
        REHAB_WORKBENCH_ID,
        "康复师工作台",
    )
}

pub fn import_from_export(
    workspace: &Path,
    request: ConnectorImportRequest,
) -> Result<crate::rehab_context::GovernedTimelineImportOutput, String> {
    let (connector_id, connector_label) = match request.source_id.as_str() {
        REHAB_WORKBENCH_ID => (REHAB_WORKBENCH_ID, "康复师工作台"),
        REHABGPT_ID => (REHABGPT_ID, "RehabGPT"),
        _ => return Err(format!("尚未实现数据源：{}", request.source_id)),
    };
    let export_path = validate_connector_export(&request.export_path, connector_id)?;
    let backup = read_backup(&export_path)?;
    import_backup(
        workspace,
        &export_path,
        backup,
        request,
        connector_id,
        connector_label,
    )
}

pub fn publish_review_feedback(
    _workspace: &Path,
    bundle: &crate::rehab_context::RehabCaseBundle,
) -> Result<bool, String> {
    let is_workbench_case = bundle.sources.iter().any(|source| {
        source.kind == "quality_report"
            && source
                .title
                .replace('\\', "/")
                .contains("output/connector-imports/rehab-workbench-")
    });
    if !is_workbench_case {
        return Ok(false);
    }

    let target = galen_bridge_snapshot_path()?.with_file_name("feedback.json");
    publish_review_feedback_to(bundle, &target)?;
    Ok(true)
}

fn publish_review_feedback_to(
    bundle: &crate::rehab_context::RehabCaseBundle,
    target: &Path,
) -> Result<(), String> {
    let mut existing = if target.is_file() {
        let bytes =
            fs::read(&target).map_err(|error| format!("读取 RehabMain 研究反馈失败: {error}"))?;
        serde_json::from_slice::<ConnectorFeedbackFile>(&bytes)
            .map_err(|error| format!("RehabMain 研究反馈格式无效: {error}"))?
    } else {
        ConnectorFeedbackFile {
            schema_version: 1,
            connector_id: REHAB_WORKBENCH_ID.into(),
            generated_at: now_millis(),
            items: Vec::new(),
        }
    };

    let observations = bundle
        .observations
        .iter()
        .map(|observation| (observation.observation_id.as_str(), observation))
        .collect::<BTreeMap<_, _>>();
    let mut items = existing
        .items
        .into_iter()
        .map(|item| (item.feedback_id.clone(), item))
        .collect::<BTreeMap<_, _>>();
    for review in &bundle.observation_reviews {
        let Some(observation) = observations.get(review.observation_id.as_str()) else {
            continue;
        };
        let Some(source_assessment_id) = observation.source_record_id.clone() else {
            continue;
        };
        let item = ConnectorFeedbackItem {
            feedback_id: review.review_id.clone(),
            rehab_id: bundle.case_record.case_id.clone(),
            source_assessment_id,
            observation_id: review.observation_id.clone(),
            metric: observation.metric.clone(),
            action: review.action,
            follow_up_action: review.follow_up_action,
            reason: review.reason.clone(),
            reviewer: review.reviewer.clone(),
            reviewed_at: review.reviewed_at.clone(),
            previous_value: review.previous_value.clone(),
            previous_unit: review.previous_unit.clone(),
            resulting_value: review.resulting_value.clone(),
            resulting_unit: review.resulting_unit.clone(),
            resulting_status: review.resulting_status,
        };
        items.insert(item.feedback_id.clone(), item);
    }
    existing.generated_at = now_millis();
    existing.items = items.into_values().collect();
    let json = serde_json::to_vec_pretty(&existing)
        .map_err(|error| format!("生成 RehabMain 研究反馈失败: {error}"))?;
    write_connector_json(&target, &json)?;
    Ok(())
}

fn write_connector_json(path: &Path, content: &[u8]) -> Result<(), String> {
    let parent = path.parent().ok_or("连接器反馈路径没有父目录")?;
    fs::create_dir_all(parent).map_err(|error| format!("创建连接器反馈目录失败: {error}"))?;
    let pending = path.with_extension("json.pending");
    fs::write(&pending, content).map_err(|error| format!("写入连接器反馈临时文件失败: {error}"))?;
    if path.exists() {
        let backup = path.with_extension("json.backup");
        let _ = fs::remove_file(&backup);
        fs::rename(path, &backup).map_err(|error| format!("备份旧连接器反馈失败: {error}"))?;
        if let Err(error) = fs::rename(&pending, path) {
            let _ = fs::rename(&backup, path);
            return Err(format!("替换连接器反馈失败: {error}"));
        }
        let _ = fs::remove_file(backup);
    } else {
        fs::rename(&pending, path).map_err(|error| format!("保存连接器反馈失败: {error}"))?;
    }
    Ok(())
}

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn import_backup(
    workspace: &Path,
    export_path: &Path,
    backup: RehabBackup,
    request: ConnectorImportRequest,
    connector_id: &str,
    connector_label: &str,
) -> Result<crate::rehab_context::GovernedTimelineImportOutput, String> {
    let selected: BTreeSet<String> = request.case_ids.into_iter().collect();
    let mut measurements = extract_measurements(&backup);
    if !selected.is_empty() {
        measurements.retain(|item| selected.contains(&item.case_id));
    }
    if let Some(limit) = request.latest_assessments.filter(|limit| *limit > 0) {
        measurements = keep_latest_timepoints(measurements, limit);
    }
    if measurements.is_empty() {
        return Err("已读取工作台数据，但所选对象没有可写入 RehabID 的数值评估记录。".into());
    }

    let import_key = format!(
        "{}-{}",
        backup.exported_at,
        short_hash(export_path.to_string_lossy().as_bytes())
    );
    let relative_dir = format!("output/connector-imports/{connector_id}-{import_key}");
    let absolute_dir = workspace.join(&relative_dir);
    fs::create_dir_all(&absolute_dir)
        .map_err(|error| format!("创建连接器导入目录失败: {error}"))?;

    let dataset_rel = format!("{relative_dir}/normalized.csv");
    let quality_rel = format!("{relative_dir}/quality-report.json");
    let dataset_text = normalized_csv(&measurements);
    fs::write(workspace.join(&dataset_rel), dataset_text)
        .map_err(|error| format!("写入连接器标准数据失败: {error}"))?;
    let source_bytes =
        fs::read(export_path).map_err(|error| format!("读取工作台导出失败: {error}"))?;
    let quality = json!({
        "schemaVersion": 1,
        "source": {
            "connectorId": connector_id,
            "application": connector_label,
            "fileName": export_path.file_name().and_then(|name| name.to_str()).unwrap_or("rehab-backup.json"),
            "sha256": format!("{:x}", Sha256::digest(&source_bytes)),
            "exportVersion": backup.version,
            "exportedAt": backup.exported_at
        },
        "governance": {
            "rawFileCopied": false,
            "identityFieldsExcluded": true,
            "normalizedMeasurementCount": measurements.len(),
            "caseIds": measurements.iter().map(|item| item.case_id.clone()).collect::<BTreeSet<_>>()
        }
    });
    fs::write(
        workspace.join(&quality_rel),
        serde_json::to_vec_pretty(&quality)
            .map_err(|error| format!("生成质量报告失败: {error}"))?,
    )
    .map_err(|error| format!("写入质量报告失败: {error}"))?;

    crate::rehab_context::import_governed_timeline(
        workspace,
        crate::rehab_context::GovernedTimelineImportInput {
            dataset_path: dataset_rel,
            quality_report_path: quality_rel,
            measurements: measurements
                .into_iter()
                .map(|item| crate::rehab_context::GovernedMeasurementInput {
                    case_id: item.case_id,
                    timepoint: item.timepoint,
                    metric: item.metric,
                    value: item.value,
                    unit: item.unit,
                    source_record_id: Some(item.assessment_id),
                    verification_status: Some(crate::rehab_context::VerificationStatus::Candidate),
                })
                .collect(),
        },
    )
}

fn latest_rehab_export(downloads: &Path) -> Result<Option<PathBuf>, String> {
    let entries = fs::read_dir(downloads).map_err(|error| format!("读取下载目录失败: {error}"))?;
    let mut candidates = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| name.starts_with("rehab-backup-") && name.ends_with(".json"))
                    .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    candidates.sort_by_key(|path| {
        fs::metadata(path)
            .and_then(|metadata| metadata.modified())
            .ok()
    });
    Ok(candidates.pop())
}

fn galen_bridge_snapshot_path() -> Result<PathBuf, String> {
    if let Ok(configured) = std::env::var("GALEN_CONNECTOR_DIR") {
        let configured = configured.trim();
        if !configured.is_empty() {
            return Ok(PathBuf::from(configured).join("latest.json"));
        }
    }
    dirs::data_local_dir()
        .map(|base| {
            base.join("Rehab")
                .join("GalenConnector")
                .join("latest.json")
        })
        .ok_or_else(|| "无法定位本机应用数据目录。".into())
}

fn rehabgpt_bridge_snapshot_path() -> Result<PathBuf, String> {
    if let Ok(configured) = std::env::var("GALEN_REHABGPT_CONNECTOR_DIR") {
        let configured = configured.trim();
        if !configured.is_empty() {
            return Ok(PathBuf::from(configured).join("latest.json"));
        }
    }
    dirs::data_local_dir()
        .map(|base| {
            base.join("RehabGPT")
                .join("GalenConnector")
                .join("latest.json")
        })
        .ok_or_else(|| "无法定位 RehabGPT 本地连接器目录。".into())
}

fn validate_connector_export(value: &str, source_id: &str) -> Result<PathBuf, String> {
    let requested =
        fs::canonicalize(value).map_err(|error| format!("找不到工作台导出文件: {error}"))?;
    if source_id == REHABGPT_ID {
        let bridge = rehabgpt_bridge_snapshot_path()?.canonicalize().ok();
        if bridge.as_ref() == Some(&requested) {
            return Ok(requested);
        }
        return Err("RehabGPT 连接器只接受其本地 Bridge 写出的 latest.json 快照。".into());
    }
    let bridge = galen_bridge_snapshot_path()?.canonicalize().ok();
    if bridge.as_ref() == Some(&requested) {
        return Ok(requested);
    }
    let downloads = dirs::download_dir().ok_or("无法定位系统下载目录。")?;
    let downloads =
        fs::canonicalize(downloads).map_err(|error| format!("无法解析系统下载目录: {error}"))?;
    let valid_name = requested
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.starts_with("rehab-backup-") && name.ends_with(".json"))
        .unwrap_or(false);
    if !requested.starts_with(downloads) || !valid_name {
        return Err("连接器只接受系统下载目录中的康复师工作台备份文件。".into());
    }
    Ok(requested)
}

fn preview_export(
    path: &Path,
    case_hint: Option<&str>,
    connection_mode: &str,
    source_id: &str,
    source_label: &str,
) -> Result<ConnectorPreview, String> {
    let backup = read_backup(path)?;
    let all_measurements = extract_measurements(&backup);
    let hint = case_hint.map(|value| value.trim().to_ascii_lowercase());
    let patient_map = patient_case_map(&backup);
    let mut cases = backup
        .patients
        .iter()
        .filter_map(|patient| {
            let source_id = text_at(patient, "id")?;
            let case_id = patient_map.get(&source_id)?.clone();
            let display_name = text_at(patient, "shortCode").unwrap_or_else(|| case_id.clone());
            if let Some(hint) = &hint {
                let searchable =
                    format!("{} {} {}", case_id, display_name, source_id).to_ascii_lowercase();
                if !searchable.contains(hint) {
                    return None;
                }
            }
            let session_count = backup
                .sessions
                .iter()
                .filter(|session| {
                    text_at(session, "patientId").as_deref() == Some(source_id.as_str())
                })
                .count();
            let assessment_count = backup
                .assessments
                .iter()
                .filter(|assessment| {
                    text_at(assessment, "patientId").as_deref() == Some(source_id.as_str())
                })
                .count();
            let measurement_count = all_measurements
                .iter()
                .filter(|measurement| measurement.case_id == case_id)
                .count();
            let timepoint_count = all_measurements
                .iter()
                .filter(|measurement| measurement.case_id == case_id)
                .map(|measurement| measurement.timepoint.as_str())
                .collect::<BTreeSet<_>>()
                .len();
            Some(ConnectorCasePreview {
                case_id,
                display_name,
                session_count,
                assessment_count,
                timepoint_count,
                measurement_count,
            })
        })
        .collect::<Vec<_>>();
    if cases.is_empty() && hint.is_some() {
        cases = build_case_previews(&backup, &all_measurements, &patient_map);
    }
    let selected_ids = cases
        .iter()
        .map(|case| case.case_id.as_str())
        .collect::<BTreeSet<_>>();
    let measurement_count = all_measurements
        .iter()
        .filter(|item| selected_ids.contains(item.case_id.as_str()))
        .count();
    let assessment_count = cases.iter().map(|case| case.assessment_count).sum();
    let timepoint_count = cases.iter().map(|case| case.timepoint_count).sum();
    let can_import = measurement_count > 0;
    let message = if can_import {
        format!(
            "已发现 {} 个 RehabID 候选、{} 个评估时间点和 {} 条数值观察。",
            cases.len(),
            timepoint_count,
            measurement_count
        )
    } else {
        "已发现对象信息，但当前备份没有可分析的数值评估记录。".into()
    };
    Ok(ConnectorPreview {
        source_id: source_id.into(),
        source_label: source_label.into(),
        export_path: path.to_string_lossy().into_owned(),
        exported_at: backup.exported_at,
        connection_mode: connection_mode.into(),
        patient_count: backup.patients.len(),
        session_count: backup.sessions.len(),
        assessment_count,
        timepoint_count,
        measurement_count,
        cases,
        can_import,
        message,
    })
}

fn build_case_previews(
    backup: &RehabBackup,
    measurements: &[ConnectorMeasurement],
    patient_map: &BTreeMap<String, String>,
) -> Vec<ConnectorCasePreview> {
    backup
        .patients
        .iter()
        .filter_map(|patient| {
            let source_id = text_at(patient, "id")?;
            let case_id = patient_map.get(&source_id)?.clone();
            Some(ConnectorCasePreview {
                display_name: text_at(patient, "shortCode").unwrap_or_else(|| case_id.clone()),
                session_count: backup
                    .sessions
                    .iter()
                    .filter(|item| {
                        text_at(item, "patientId").as_deref() == Some(source_id.as_str())
                    })
                    .count(),
                assessment_count: backup
                    .assessments
                    .iter()
                    .filter(|item| {
                        text_at(item, "patientId").as_deref() == Some(source_id.as_str())
                    })
                    .count(),
                timepoint_count: measurements
                    .iter()
                    .filter(|item| item.case_id == case_id)
                    .map(|item| item.timepoint.as_str())
                    .collect::<BTreeSet<_>>()
                    .len(),
                measurement_count: measurements
                    .iter()
                    .filter(|item| item.case_id == case_id)
                    .count(),
                case_id,
            })
        })
        .collect()
}

fn read_backup(path: &Path) -> Result<RehabBackup, String> {
    let bytes = fs::read(path).map_err(|error| format!("读取康复师工作台导出失败: {error}"))?;
    if bytes.len() > 100 * 1024 * 1024 {
        return Err("工作台导出超过 100MB，请按对象或时间范围拆分。".into());
    }
    let backup: RehabBackup = serde_json::from_slice(&bytes)
        .map_err(|error| format!("康复师工作台备份格式无效: {error}"))?;
    if backup.version.trim().is_empty() {
        return Err("康复师工作台备份缺少版本号。".into());
    }
    Ok(backup)
}

fn patient_case_map(backup: &RehabBackup) -> BTreeMap<String, String> {
    backup
        .patients
        .iter()
        .filter_map(|patient| {
            let source_id = text_at(patient, "id")?;
            // Human names are not research identifiers. If the source has no
            // explicit short code, persist a stable pseudonym derived from the
            // opaque upstream record ID instead of leaking identity into RehabID.
            let candidate = text_at(patient, "shortCode")
                .unwrap_or_else(|| format!("RID-{}", short_hash(source_id.as_bytes())));
            Some((source_id, normalize_id(&candidate)))
        })
        .collect()
}

fn extract_measurements(backup: &RehabBackup) -> Vec<ConnectorMeasurement> {
    let patient_map = patient_case_map(backup);
    let mut output = Vec::new();
    for assessment in &backup.assessments {
        let Some(patient_id) = text_at(assessment, "patientId") else {
            continue;
        };
        let Some(case_id) = patient_map.get(&patient_id).cloned() else {
            continue;
        };
        let assessment_id = text_at(assessment, "id").unwrap_or_else(|| "assessment".into());
        let created_at = assessment
            .get("createdAt")
            .and_then(Value::as_u64)
            .unwrap_or(backup.exported_at);
        let timepoint = connector_timepoint(assessment, created_at);
        let mut push = |metric: String, value: f64, unit: String| {
            if value.is_finite() {
                output.push(ConnectorMeasurement {
                    case_id: case_id.clone(),
                    timepoint: timepoint.clone(),
                    metric: normalize_id(&metric),
                    value,
                    unit,
                    assessment_id: assessment_id.clone(),
                    created_at,
                });
            }
        };

        if let Some(metrics) = assessment.get("summaryMetrics").and_then(Value::as_array) {
            for metric in metrics {
                if let (Some(key), Some(value)) = (
                    text_at(metric, "metricKey"),
                    metric.get("value").and_then(Value::as_f64),
                ) {
                    push(
                        key,
                        value,
                        text_at(metric, "unit").unwrap_or_else(|| "value".into()),
                    );
                }
            }
        }
        if let Some(metrics) = assessment
            .pointer("/data/posture/metrics")
            .and_then(Value::as_object)
        {
            for (key, value) in metrics {
                if let Some(value) = value.as_f64() {
                    push(format!("posture_{key}"), value, inferred_unit(key));
                }
            }
        }
        if let Some(items) = assessment
            .pointer("/data/rom/items")
            .and_then(Value::as_array)
        {
            for item in items {
                if let Some(value) = item.get("angle").and_then(Value::as_f64) {
                    let metric = format!(
                        "rom_{}_{}_{}",
                        text_at(item, "joint").unwrap_or_else(|| "joint".into()),
                        text_at(item, "direction").unwrap_or_else(|| "motion".into()),
                        text_at(item, "side").unwrap_or_else(|| "side".into())
                    );
                    push(metric, value, "deg".into());
                }
            }
        }
        if let Some(scale) = assessment.pointer("/data/scale") {
            let prefix = text_at(scale, "scaleId")
                .unwrap_or_else(|| "scale".into())
                .to_ascii_lowercase();
            for (key, unit) in [("totalScore", "score"), ("percentageScore", "%")] {
                if let Some(value) = scale.get(key).and_then(Value::as_f64) {
                    push(format!("{prefix}_{key}"), value, unit.into());
                }
            }
            if let Some(dimensions) = scale.get("dimensions").and_then(Value::as_object) {
                for (key, value) in dimensions {
                    if let Some(value) = value.as_f64() {
                        push(format!("{prefix}_{key}"), value, "score".into());
                    }
                }
            }
            for items_key in ["items", "scaleItems", "scale_items"] {
                if let Some(items) = scale.get(items_key).and_then(Value::as_array) {
                    for item in items {
                        let key = text_at(item, "item")
                            .or_else(|| text_at(item, "key"))
                            .or_else(|| text_at(item, "id"))
                            .or_else(|| text_at(item, "name"))
                            .or_else(|| text_at(item, "label"));
                        let value = ["score", "value", "rawScore", "raw_score"]
                            .iter()
                            .find_map(|field| item.get(*field).and_then(Value::as_f64));
                        if let (Some(key), Some(value)) = (key, value) {
                            push(format!("{prefix}_{key}"), value, "score".into());
                        }
                    }
                }
            }
        }
        if let Some(adams) = assessment.pointer("/data/adams") {
            // Observed ATR/scoliometer readings can enter review. A model or
            // mobile estimated Cobb angle is diagnosis-like derived output and
            // must not become a governed clinical observation.
            for key in ["atrDegrees", "scoliometerReading"] {
                if let Some(value) = adams.get(key).and_then(Value::as_f64) {
                    push(format!("adams_{key}"), value, "deg".into());
                }
            }
        }
        for container in [assessment.get("metrics"), assessment.get("angles")] {
            if let Some(values) = container.and_then(Value::as_object) {
                for (key, value) in values {
                    if let Some(value) = value.as_f64() {
                        push(key.clone(), value, inferred_unit(key));
                    }
                }
            }
        }
    }
    output.sort_by(|left, right| {
        (&left.case_id, left.created_at, &left.metric).cmp(&(
            &right.case_id,
            right.created_at,
            &right.metric,
        ))
    });
    output.dedup_by(|left, right| {
        left.case_id == right.case_id
            && left.timepoint == right.timepoint
            && left.metric == right.metric
    });
    output
}

/// Preserve a meaningful study visit label when the upstream workbench has one.
/// Generic exports without a session convention retain their timestamp as the
/// stable fallback, so this does not reinterpret arbitrary clinical records.
fn connector_timepoint(assessment: &Value, created_at: u64) -> String {
    if assessment
        .get("isBaseline")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return "baseline".into();
    }
    let session_id = text_at(assessment, "sessionId").unwrap_or_default();
    if session_id.ends_with("-T0") {
        "T0_30min".into()
    } else if session_id.ends_with("-T24") {
        "T24h".into()
    } else if session_id.ends_with("-T72") {
        "T72h".into()
    } else {
        created_at.to_string()
    }
}

fn keep_latest_timepoints(
    measurements: Vec<ConnectorMeasurement>,
    limit: usize,
) -> Vec<ConnectorMeasurement> {
    let mut by_case: BTreeMap<String, BTreeMap<String, u64>> = BTreeMap::new();
    for item in &measurements {
        by_case
            .entry(item.case_id.clone())
            .or_default()
            .entry(item.timepoint.clone())
            .and_modify(|created_at| *created_at = (*created_at).max(item.created_at))
            .or_insert(item.created_at);
    }
    let keep = by_case
        .into_iter()
        .flat_map(|(case_id, timepoints)| {
            let mut timepoints = timepoints.into_iter().collect::<Vec<_>>();
            timepoints.sort_by_key(|(_, created_at)| *created_at);
            timepoints
                .into_iter()
                .rev()
                .take(limit)
                .map(move |(timepoint, _)| (case_id.clone(), timepoint))
        })
        .collect::<BTreeSet<_>>();
    measurements
        .into_iter()
        .filter(|item| keep.contains(&(item.case_id.clone(), item.timepoint.clone())))
        .collect()
}

fn normalized_csv(measurements: &[ConnectorMeasurement]) -> String {
    let mut text = String::from("rehab_id,timepoint,metric,value,unit,source_assessment_id\n");
    for item in measurements {
        text.push_str(&format!(
            "{},{},{},{},{},{}\n",
            csv(&item.case_id),
            csv(&item.timepoint),
            csv(&item.metric),
            item.value,
            csv(&item.unit),
            csv(&item.assessment_id)
        ));
    }
    text
}

fn csv(value: &str) -> String {
    if value
        .chars()
        .any(|character| matches!(character, ',' | '"' | '\n' | '\r'))
    {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.into()
    }
}

fn text_at(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)?
        .as_str()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

fn normalize_id(value: &str) -> String {
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
        .collect::<String>();
    let normalized = normalized
        .trim_matches('-')
        .chars()
        .take(80)
        .collect::<String>();
    if normalized.is_empty() {
        format!("RID-{}", short_hash(value.as_bytes()))
    } else {
        normalized
    }
}

fn inferred_unit(key: &str) -> String {
    let key = key.to_ascii_lowercase();
    if key.contains("angle")
        || key.contains("tilt")
        || key.contains("rotation")
        || key.contains("atr")
    {
        "deg".into()
    } else if key.contains("percent") || key.contains("score_pct") {
        "%".into()
    } else if key.contains("cm") || key.contains("height") || key.contains("offset") {
        "cm".into()
    } else {
        "value".into()
    }
}

fn short_hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
        .chars()
        .take(10)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(tag: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("galen-connector-{tag}-{nonce}"));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn fixture() -> &'static str {
        r#"{"version":"1.0.0","exportedAt":1789000000000,"patients":[{"id":"patient-1","shortCode":"ATH-001","name":"测试对象"}],"sessions":[{"id":"s1","patientId":"patient-1"}],"assessments":[{"id":"a1","patientId":"patient-1","createdAt":1788990000000,"isBaseline":true,"data":{"posture":{"metrics":{"shoulderAngle":3.2}}}},{"id":"a2","patientId":"patient-1","createdAt":1788995000000,"data":{"scale":{"scaleId":"VAS","totalScore":6,"percentageScore":60,"dimensions":{"pain":6}}}},{"id":"a3","patientId":"patient-1","createdAt":1788999000000,"data":{"adams":{"atrDegrees":8,"cobbAngleEstimate":14}}}]}"#
    }

    #[test]
    fn previews_real_rehab_backup_shape() {
        let dir = temp_dir("preview");
        let path = dir.join("rehab-backup-2026-09-10.json");
        fs::write(&path, fixture()).unwrap();
        let preview = preview_export(
            &path,
            Some("ATH-001"),
            "file_fallback",
            REHAB_WORKBENCH_ID,
            "康复师工作台",
        )
        .unwrap();
        assert_eq!(preview.patient_count, 1);
        assert_eq!(preview.assessment_count, 3);
        assert_eq!(preview.timepoint_count, 3);
        assert_eq!(preview.measurement_count, 5);
        assert!(preview.can_import);
        assert_eq!(preview.cases[0].case_id, "ATH-001");
        let backup = read_backup(&path).unwrap();
        assert!(!extract_measurements(&backup)
            .iter()
            .any(|item| item.metric == "adams_cobbAngleEstimate"));
    }

    #[test]
    fn missing_short_code_uses_pseudonym_and_never_displays_name() {
        let dir = temp_dir("pseudonym");
        let path = dir.join("rehab-backup-private.json");
        fs::write(
            &path,
            r#"{"version":"1.0.0","exportedAt":1789000000000,"patients":[{"id":"opaque-patient-42","name":"真实姓名"}],"sessions":[],"assessments":[{"id":"a1","patientId":"opaque-patient-42","createdAt":1788990000000,"metrics":{"pain":4}}]}"#,
        )
        .unwrap();

        let preview = preview_export(
            &path,
            None,
            "file_fallback",
            REHAB_WORKBENCH_ID,
            "康复师工作台",
        )
        .unwrap();

        assert_eq!(preview.cases.len(), 1);
        assert!(preview.cases[0].case_id.starts_with("RID-"));
        assert_eq!(preview.cases[0].display_name, preview.cases[0].case_id);
        assert!(!serde_json::to_string(&preview)
            .unwrap()
            .contains("真实姓名"));
    }

    #[test]
    fn connector_import_marks_observations_candidate_and_keeps_record_provenance() {
        let workspace = temp_dir("candidate-workspace");
        let export = workspace.join("rehab-backup-source.json");
        fs::write(&export, fixture()).unwrap();
        let backup = read_backup(&export).unwrap();
        let output = import_backup(
            &workspace,
            &export,
            backup,
            ConnectorImportRequest {
                source_id: REHAB_WORKBENCH_ID.into(),
                export_path: export.to_string_lossy().into_owned(),
                case_ids: vec!["ATH-001".into()],
                latest_assessments: None,
            },
            REHAB_WORKBENCH_ID,
            "康复师工作台",
        )
        .unwrap();

        assert_eq!(output.imported_observation_count, 5);
        assert_eq!(output.candidate_observation_count, 5);
        let bundle = crate::rehab_context::load_case_bundle(&workspace, "ATH-001").unwrap();
        assert!(bundle.observations.iter().all(|item| {
            item.verification_status == crate::rehab_context::VerificationStatus::Candidate
        }));
        assert!(bundle.observations.iter().all(|item| item
            .note
            .as_deref()
            .unwrap_or_default()
            .contains("上游来源记录")));
        assert!(bundle
            .observations
            .iter()
            .all(|item| item.source_record_id.is_some()));
        assert!(!bundle
            .observations
            .iter()
            .any(|item| item.metric.contains("cobb")));
    }

    #[test]
    fn preserves_workbench_visit_labels_for_longitudinal_analysis() {
        let baseline = serde_json::json!({"isBaseline": true, "sessionId": "ATH-001-D7"});
        let immediate = serde_json::json!({"sessionId": "ATH-001-T0"});
        let day_one = serde_json::json!({"sessionId": "ATH-001-T24"});
        let day_three = serde_json::json!({"sessionId": "ATH-001-T72"});

        assert_eq!(connector_timepoint(&baseline, 1), "baseline");
        assert_eq!(connector_timepoint(&immediate, 2), "T0_30min");
        assert_eq!(connector_timepoint(&day_one, 3), "T24h");
        assert_eq!(connector_timepoint(&day_three, 4), "T72h");
    }

    #[test]
    fn extracts_scale_item_scores_for_subscale_analysis() {
        let mut backup: RehabBackup = serde_json::from_str(fixture()).unwrap();
        backup.assessments.push(serde_json::from_str(
            r#"{"id":"a4","patientId":"patient-1","createdAt":1789000000000,"data":{"scale":{"scaleId":"FMA-UE","totalScore":42,"scale_items":[{"item":"wrist_flexion","score":2},{"item":"grip","score":1}]}}}"#,
        ).unwrap());

        let measurements = extract_measurements(&backup);

        assert!(measurements
            .iter()
            .any(|item| item.metric == "fma-ue_wrist_flexion" && item.value == 2.0));
        assert!(measurements
            .iter()
            .any(|item| item.metric == "fma-ue_grip" && item.value == 1.0));
    }

    #[test]
    fn discovers_the_live_bridge_snapshot_before_file_fallback() {
        let dir = temp_dir("live-bridge");
        fs::write(dir.join("latest.json"), fixture()).unwrap();
        let previous = std::env::var_os("GALEN_CONNECTOR_DIR");
        std::env::set_var("GALEN_CONNECTOR_DIR", &dir);
        let preview = discover_latest(REHAB_WORKBENCH_ID, Some("ATH-001")).unwrap();
        match previous {
            Some(value) => std::env::set_var("GALEN_CONNECTOR_DIR", value),
            None => std::env::remove_var("GALEN_CONNECTOR_DIR"),
        }
        assert_eq!(preview.connection_mode, "live_bridge");
        assert_eq!(
            preview.export_path,
            dir.join("latest.json").to_string_lossy()
        );
        assert_eq!(preview.measurement_count, 5);
    }

    #[test]
    fn discovers_a_rehabgpt_bridge_as_a_named_research_source() {
        let dir = temp_dir("rehabgpt-bridge");
        fs::write(dir.join("latest.json"), fixture()).unwrap();
        let previous = std::env::var_os("GALEN_REHABGPT_CONNECTOR_DIR");
        std::env::set_var("GALEN_REHABGPT_CONNECTOR_DIR", &dir);
        let preview = discover_latest("rehabgpt", Some("ATH-001")).unwrap();
        match previous {
            Some(value) => std::env::set_var("GALEN_REHABGPT_CONNECTOR_DIR", value),
            None => std::env::remove_var("GALEN_REHABGPT_CONNECTOR_DIR"),
        }
        assert_eq!(preview.source_id, "rehabgpt");
        assert_eq!(preview.source_label, "RehabGPT");
        assert_eq!(preview.connection_mode, "live_bridge");
        assert_eq!(preview.measurement_count, 5);
    }

    #[test]
    fn latest_filter_keeps_every_module_from_the_same_timepoint() {
        let mut backup: RehabBackup = serde_json::from_str(fixture()).unwrap();
        backup.assessments.push(serde_json::from_str(
            r#"{"id":"a4","patientId":"patient-1","createdAt":1788999000000,"data":{"scale":{"scaleId":"RPE","totalScore":8}}}"#,
        ).unwrap());
        let all = extract_measurements(&backup);
        let latest = keep_latest_timepoints(all, 1);
        assert_eq!(latest.len(), 2);
        assert!(latest.iter().all(|item| item.created_at == 1788999000000));
        assert_eq!(
            latest
                .iter()
                .map(|item| item.assessment_id.as_str())
                .collect::<BTreeSet<_>>(),
            BTreeSet::from(["a3", "a4"])
        );
    }

    #[test]
    fn empty_assessment_backup_is_honest() {
        let dir = temp_dir("empty");
        let path = dir.join("rehab-backup-2026-09-10.json");
        fs::write(&path, r#"{"version":"1.0.0","exportedAt":1,"patients":[{"id":"p1","shortCode":"RID-1"}],"sessions":[],"assessments":[]}"#).unwrap();
        let preview = preview_export(
            &path,
            None,
            "file_fallback",
            REHAB_WORKBENCH_ID,
            "康复师工作台",
        )
        .unwrap();
        assert!(!preview.can_import);
        assert_eq!(preview.measurement_count, 0);
    }

    #[test]
    fn connector_import_builds_a_durable_rehabid_timeline() {
        let workspace = temp_dir("workspace");
        let export_dir = temp_dir("export");
        let export_path = export_dir.join("rehab-backup-2026-09-10.json");
        fs::write(&export_path, fixture()).unwrap();
        let backup = read_backup(&export_path).unwrap();
        let output = import_backup(
            &workspace,
            &export_path,
            backup,
            ConnectorImportRequest {
                source_id: REHAB_WORKBENCH_ID.into(),
                export_path: export_path.to_string_lossy().into_owned(),
                case_ids: vec!["ATH-001".into()],
                latest_assessments: Some(3),
            },
            REHAB_WORKBENCH_ID,
            "康复师工作台",
        )
        .unwrap();
        assert_eq!(output.case_ids, vec!["ATH-001"]);
        assert_eq!(output.imported_event_count, 3);
        assert_eq!(output.imported_observation_count, 5);
        assert_eq!(output.candidate_observation_count, 5);
        let bundle = crate::rehab_context::load_case_bundle(&workspace, "ATH-001").unwrap();
        assert_eq!(bundle.events.len(), 3);
        assert_eq!(bundle.observations.len(), 5);
        assert!(workspace.join(output.receipt.path).is_file());
    }

    #[test]
    fn reviewed_observation_becomes_idempotent_rehabmain_feedback() {
        let workspace = temp_dir("feedback-workspace");
        let export = workspace.join("rehab-backup-source.json");
        fs::write(&export, fixture()).unwrap();
        let backup = read_backup(&export).unwrap();
        import_backup(
            &workspace,
            &export,
            backup,
            ConnectorImportRequest {
                source_id: REHAB_WORKBENCH_ID.into(),
                export_path: export.to_string_lossy().into_owned(),
                case_ids: vec!["ATH-001".into()],
                latest_assessments: None,
            },
            REHAB_WORKBENCH_ID,
            "康复师工作台",
        )
        .unwrap();
        let initial = crate::rehab_context::load_case_bundle(&workspace, "ATH-001").unwrap();
        let observation = initial
            .observations
            .iter()
            .find(|item| item.source_record_id.as_deref() == Some("a3"))
            .unwrap();
        let reviewed = crate::rehab_context::review_observation(
            &workspace,
            crate::rehab_context::ObservationReviewInput {
                case_id: "ATH-001".into(),
                observation_id: observation.observation_id.clone(),
                expected_revision: initial.revision,
                action: crate::rehab_context::ObservationReviewAction::Reject,
                reason: "该时间点需重新采集".into(),
                reviewer: "researcher-1".into(),
                corrected_value: None,
                corrected_unit: None,
                follow_up_action: crate::rehab_context::ResearchFollowUpAction::Recapture,
            },
        )
        .unwrap();
        let feedback_path = workspace.join("feedback.json");
        publish_review_feedback_to(&reviewed, &feedback_path).unwrap();
        publish_review_feedback_to(&reviewed, &feedback_path).unwrap();

        let feedback: ConnectorFeedbackFile =
            serde_json::from_slice(&fs::read(feedback_path).unwrap()).unwrap();
        assert_eq!(feedback.items.len(), 1);
        assert_eq!(feedback.items[0].source_assessment_id, "a3");
        assert_eq!(feedback.items[0].rehab_id, "ATH-001");
        assert_eq!(
            feedback.items[0].follow_up_action,
            crate::rehab_context::ResearchFollowUpAction::Recapture
        );
    }
}
