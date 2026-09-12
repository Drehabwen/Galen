use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

const REHAB_WORKBENCH_ID: &str = "rehab-workbench";

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

pub fn discover_latest(
    source_id: &str,
    case_hint: Option<&str>,
) -> Result<ConnectorPreview, String> {
    if source_id != REHAB_WORKBENCH_ID {
        return Err(format!("尚未实现数据源：{source_id}"));
    }
    let bridge = galen_bridge_snapshot_path()?;
    if bridge.is_file() {
        return preview_export(&bridge, case_hint, "live_bridge");
    }
    let downloads = dirs::download_dir().ok_or("无法定位系统下载目录。")?;
    let export = latest_rehab_export(&downloads)?.ok_or(
        "尚未发现康复师工作台导出。请在工作台的「系统设置 → 数据备份」中导出一次完整数据。",
    )?;
    preview_export(&export, case_hint, "file_fallback")
}

pub fn import_from_export(
    workspace: &Path,
    request: ConnectorImportRequest,
) -> Result<crate::rehab_context::GovernedTimelineImportOutput, String> {
    if request.source_id != REHAB_WORKBENCH_ID {
        return Err(format!("尚未实现数据源：{}", request.source_id));
    }
    let export_path = validate_connector_export(&request.export_path)?;
    let backup = read_backup(&export_path)?;
    import_backup(workspace, &export_path, backup, request)
}

fn import_backup(
    workspace: &Path,
    export_path: &Path,
    backup: RehabBackup,
    request: ConnectorImportRequest,
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
    let relative_dir = format!("output/connector-imports/rehab-workbench-{import_key}");
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
            "connectorId": REHAB_WORKBENCH_ID,
            "application": "康复师工作台",
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

fn validate_connector_export(value: &str) -> Result<PathBuf, String> {
    let requested =
        fs::canonicalize(value).map_err(|error| format!("找不到工作台导出文件: {error}"))?;
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
            let display_name = text_at(patient, "name").unwrap_or_else(|| case_id.clone());
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
        source_id: REHAB_WORKBENCH_ID.into(),
        source_label: "康复师工作台".into(),
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
                display_name: text_at(patient, "name").unwrap_or_else(|| case_id.clone()),
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
            let candidate = text_at(patient, "shortCode")
                .or_else(|| text_at(patient, "name"))
                .unwrap_or_else(|| source_id.clone());
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
        }
        if let Some(adams) = assessment.pointer("/data/adams") {
            for key in ["atrDegrees", "cobbAngleEstimate", "scoliometerReading"] {
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
        let preview = preview_export(&path, Some("ATH-001"), "file_fallback").unwrap();
        assert_eq!(preview.patient_count, 1);
        assert_eq!(preview.assessment_count, 3);
        assert_eq!(preview.timepoint_count, 3);
        assert_eq!(preview.measurement_count, 6);
        assert!(preview.can_import);
        assert_eq!(preview.cases[0].case_id, "ATH-001");
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
        assert_eq!(preview.measurement_count, 6);
    }

    #[test]
    fn latest_filter_keeps_every_module_from_the_same_timepoint() {
        let mut backup: RehabBackup = serde_json::from_str(fixture()).unwrap();
        backup.assessments.push(serde_json::from_str(
            r#"{"id":"a4","patientId":"patient-1","createdAt":1788999000000,"data":{"scale":{"scaleId":"RPE","totalScore":8}}}"#,
        ).unwrap());
        let all = extract_measurements(&backup);
        let latest = keep_latest_timepoints(all, 1);
        assert_eq!(latest.len(), 3);
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
        let preview = preview_export(&path, None, "file_fallback").unwrap();
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
        )
        .unwrap();
        assert_eq!(output.case_ids, vec!["ATH-001"]);
        assert_eq!(output.imported_event_count, 3);
        assert_eq!(output.imported_observation_count, 6);
        let bundle = crate::rehab_context::load_case_bundle(&workspace, "ATH-001").unwrap();
        assert_eq!(bundle.events.len(), 3);
        assert_eq!(bundle.observations.len(), 6);
        assert!(workspace.join(output.receipt.path).is_file());
    }
}
