use super::{ConnectorMeasurement, RehabBackup};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub(super) fn read_backup(path: &Path) -> Result<RehabBackup, String> {
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

pub(super) fn patient_case_map(backup: &RehabBackup) -> BTreeMap<String, String> {
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

pub(super) fn extract_measurements(backup: &RehabBackup) -> Vec<ConnectorMeasurement> {
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
pub(super) fn connector_timepoint(assessment: &Value, created_at: u64) -> String {
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

pub(super) fn keep_latest_timepoints(
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

pub(super) fn normalized_csv(measurements: &[ConnectorMeasurement]) -> String {
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

pub(super) fn csv(value: &str) -> String {
    if value
        .chars()
        .any(|character| matches!(character, ',' | '"' | '\n' | '\r'))
    {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.into()
    }
}

pub(super) fn text_at(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)?
        .as_str()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

pub(super) fn normalize_id(value: &str) -> String {
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

pub(super) fn inferred_unit(key: &str) -> String {
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

pub(super) fn short_hash(bytes: &[u8]) -> String {
