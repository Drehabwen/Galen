use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::rehab_context::{
    import_ais_case, list_case_summaries, load_case_bundle, resolve_review, CohortStatus,
    VerificationStatus,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JourneyCheck {
    pub name: String,
    pub passed: bool,
    pub expected: String,
    pub actual: String,
    pub critical: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JourneyStep {
    pub actor: String,
    pub action: String,
    pub observable: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GoldenJourneyResult {
    pub journey_id: String,
    pub title: String,
    pub persona: String,
    pub passed: bool,
    pub duration_ms: u128,
    pub steps: Vec<JourneyStep>,
    pub checks: Vec<JourneyCheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvalMetric {
    pub id: String,
    pub label: String,
    pub value: f64,
    pub threshold: f64,
    pub passed: bool,
    pub unit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RehabGoldenEvalReport {
    pub schema_version: u32,
    pub suite_id: String,
    pub generated_at: String,
    pub passed: bool,
    pub negative_optimization_detected: bool,
    pub journeys: Vec<GoldenJourneyResult>,
    pub metrics: Vec<EvalMetric>,
    pub recommendations: Vec<String>,
}

pub fn run_golden_journeys(
    workspace: &Path,
    source_path: &str,
) -> Result<RehabGoldenEvalReport, String> {
    let source =
        crate::tools::workspace_path::resolve_workspace_path_from_root(workspace, source_path)?;
    let run_root = std::env::temp_dir().join(format!("galen-golden-journeys-{}", nonce()));
    std::fs::create_dir_all(&run_root)
        .map_err(|error| format!("创建隔离评测工作区失败: {error}"))?;
    std::fs::copy(&source, run_root.join("cases.json"))
        .map_err(|error| format!("复制评测病例集失败: {error}"))?;

    let mut journeys = Vec::new();

    let started = Instant::now();
    let c025 = import_ais_case(&run_root, "cases.json", "AIS-C025")?;
    journeys.push(journey(
        "J01",
        "冷启动导入真实纵向病例",
        "第一次使用 Galen 的康复科研人员",
        started,
        vec![
            step(
                "用户",
                "选择病例集并输入 AIS-C025",
                "Galen 创建去标识化病例记录",
            ),
            step(
                "Galen",
                "解析来源、事件和观察值",
                "形成 3 个时间点的证据脉络",
            ),
            step("用户", "查看队列结果", "无需离开 Galen 即可看到纵向变化"),
        ],
        vec![
            check(
                "病例身份",
                c025.case_record.case_id == "AIS-C025",
                "AIS-C025",
                &c025.case_record.case_id,
                true,
            ),
            check(
                "事件数量",
                c025.events.len() == 3,
                "3",
                &c025.events.len().to_string(),
                true,
            ),
            number_check(&c025, "cobb_deg_thoracic_baseline", 45.0, true),
            number_check(&c025, "cobb_deg_thoracic_follow_up", 26.0, true),
            number_check(&c025, "cobb_deg_thoracic_change", -19.0, true),
        ],
    ));

    let started = Instant::now();
    let disputed = c025
        .observations
        .iter()
        .find(|observation| observation.observation_id == "C025-I-T");
    journeys.push(journey(
        "J02",
        "用户暂不处理来源冲突",
        "希望先看到可用结果、稍后再裁决的忙碌治疗师",
        started,
        vec![
            step("Galen", "发现正文 7° 与图注 -6° 冲突", "保留两个候选来源"),
            step("用户", "暂不裁决", "冲突值不进入硬门，但主纵向结果仍可生成"),
        ],
        vec![
            check(
                "冲突保持开放",
                c025.cohort_row.open_review_count == 1,
                "1",
                &c025.cohort_row.open_review_count.to_string(),
                true,
            ),
            check(
                "冲突值不进入硬门",
                !c025
                    .cohort_row
                    .selected_observation_ids
                    .iter()
                    .any(|id| id == "C025-I-T"),
                "excluded",
                if c025
                    .cohort_row
                    .selected_observation_ids
                    .iter()
                    .any(|id| id == "C025-I-T")
                {
                    "included"
                } else {
                    "excluded"
                },
                true,
            ),
            check(
                "冲突状态可见",
                disputed
                    .map(|item| item.verification_status == VerificationStatus::Disputed)
                    .unwrap_or(false),
                "disputed",
                disputed
                    .map(|item| format!("{:?}", item.verification_status))
                    .unwrap_or_else(|| "missing".into())
                    .as_str(),
                true,
            ),
            check(
                "不阻塞无关主结局",
                c025.cohort_row.status == CohortStatus::Included,
                "included",
                &format!("{:?}", c025.cohort_row.status),
                true,
            ),
        ],
    ));

    let started = Instant::now();
    let resolved = resolve_review(
        &run_root,
        "AIS-C025",
        "C025-CONFLICT-INBRACE-T",
        "option-1",
        "golden-journey-reviewer",
    )?;
    let resolved_observation = resolved
        .observations
        .iter()
        .find(|observation| observation.observation_id == "C025-I-T");
    journeys.push(journey(
        "J03",
        "人工局部裁决并自动复算",
        "核对原书后选择正文值的研究者",
        started,
        vec![
            step("用户", "选择正文值 7°", "仅更新目标观察值"),
            step(
                "Galen",
                "提升 revision 并重新生成队列行",
                "所有依赖状态保持一致",
            ),
        ],
        vec![
            check(
                "版本提升",
                resolved.revision == 2,
                "2",
                &resolved.revision.to_string(),
                true,
            ),
            check(
                "裁决关闭",
                resolved.cohort_row.open_review_count == 0,
                "0",
                &resolved.cohort_row.open_review_count.to_string(),
                true,
            ),
            check(
                "观察值核验",
                resolved_observation
                    .map(|item| {
                        item.verification_status == VerificationStatus::Verified
                            && item.value.as_ref().and_then(Value::as_f64) == Some(7.0)
                    })
                    .unwrap_or(false),
                "verified:7",
                &resolved_observation
                    .map(|item| format!("{:?}:{:?}", item.verification_status, item.value))
                    .unwrap_or_else(|| "missing".into()),
                true,
            ),
            number_check(&resolved, "cobb_deg_thoracic_change", -19.0, true),
        ],
    ));

    let started = Instant::now();
    let c021 = import_ais_case(&run_root, "cases.json", "AIS-C021")?;
    journeys.push(journey(
        "J04",
        "胸弯与腰弯区域隔离",
        "检查多曲线病例的临床研究者",
        started,
        vec![
            step(
                "用户",
                "导入包含胸弯和腰弯的 AIS-C021",
                "两个区域分别形成纵向配对",
            ),
            step("Galen", "按 metric + region 配对", "不跨区域拼接观察值"),
        ],
        vec![
            number_check(&c021, "cobb_deg_thoracic_change", -7.0, true),
            number_check(&c021, "cobb_deg_lumbar_change", -20.0, true),
            check(
                "区域结果相互独立",
                c021.cohort_row
                    .derived_values
                    .contains_key("cobb_deg_thoracic_change")
                    && c021
                        .cohort_row
                        .derived_values
                        .contains_key("cobb_deg_lumbar_change"),
                "thoracic+lumbar",
                &format!("{} keys", c021.cohort_row.derived_values.len()),
                true,
            ),
        ],
    ));

    let started = Instant::now();
    let restored = load_case_bundle(&run_root, "AIS-C025")?;
    let summaries = list_case_summaries(&run_root)?;
    journeys.push(journey(
        "J05",
        "关闭应用后恢复上一轮状态",
        "中断工作、稍后继续的单人研究者",
        started,
        vec![
            step("用户", "关闭并重新打开 Galen", "内存状态被清空"),
            step("Galen", "从工作区恢复病例", "裁决、版本和队列结果完整恢复"),
        ],
        vec![
            check(
                "病例列表恢复",
                summaries.len() == 2,
                "2",
                &summaries.len().to_string(),
                true,
            ),
            check(
                "版本恢复",
                restored.revision == 2,
                "2",
                &restored.revision.to_string(),
                true,
            ),
            check(
                "裁决恢复",
                restored.cohort_row.open_review_count == 0,
                "0",
                &restored.cohort_row.open_review_count.to_string(),
                true,
            ),
            number_check(&restored, "cobb_deg_thoracic_change", -19.0, true),
        ],
    ));

    let total_checks = journeys.iter().map(|item| item.checks.len()).sum::<usize>();
    let passed_checks = journeys
        .iter()
        .flat_map(|item| &item.checks)
        .filter(|check| check.passed)
        .count();
    let critical_checks = journeys
        .iter()
        .flat_map(|item| &item.checks)
        .filter(|check| check.critical)
        .collect::<Vec<_>>();
    let critical_passed = critical_checks.iter().filter(|check| check.passed).count();
    let journey_pass_rate =
        journeys.iter().filter(|item| item.passed).count() as f64 / journeys.len() as f64;
    let check_accuracy = passed_checks as f64 / total_checks as f64;
    let critical_accuracy = critical_passed as f64 / critical_checks.len() as f64;
    let source_coverage = (c025.cohort_row.source_coverage + c021.cohort_row.source_coverage) / 2.0;
    let recovery = if journeys.last().map(|item| item.passed).unwrap_or(false) {
        1.0
    } else {
        0.0
    };
    let metrics = vec![
        metric(
            "journey_pass_rate",
            "黄金旅程完成率",
            journey_pass_rate,
            0.9,
            "ratio",
        ),
        metric(
            "observable_check_accuracy",
            "可观测状态准确率",
            check_accuracy,
            0.98,
            "ratio",
        ),
        metric(
            "critical_fact_accuracy",
            "关键事实准确率",
            critical_accuracy,
            1.0,
            "ratio",
        ),
        metric(
            "source_coverage",
            "来源覆盖率",
            source_coverage,
            0.95,
            "ratio",
        ),
        metric("state_recovery", "状态恢复率", recovery, 1.0, "ratio"),
    ];
    let negative_optimization_detected = metrics.iter().any(|metric| !metric.passed);
    let recommendations = build_recommendations(&journeys, &metrics);

    let _ = std::fs::remove_dir_all(&run_root);
    Ok(RehabGoldenEvalReport {
        schema_version: 1,
        suite_id: "ais-longitudinal-golden-v1".into(),
        generated_at: now_timestamp(),
        passed: !negative_optimization_detected,
        negative_optimization_detected,
        journeys,
        metrics,
        recommendations,
    })
}

fn journey(
    journey_id: &str,
    title: &str,
    persona: &str,
    started: Instant,
    steps: Vec<JourneyStep>,
    checks: Vec<JourneyCheck>,
) -> GoldenJourneyResult {
    GoldenJourneyResult {
        journey_id: journey_id.into(),
        title: title.into(),
        persona: persona.into(),
        passed: checks.iter().all(|check| check.passed),
        duration_ms: started.elapsed().as_millis(),
        steps,
        checks,
    }
}

fn step(actor: &str, action: &str, observable: &str) -> JourneyStep {
    JourneyStep {
        actor: actor.into(),
        action: action.into(),
        observable: observable.into(),
    }
}

fn check(name: &str, passed: bool, expected: &str, actual: &str, critical: bool) -> JourneyCheck {
    JourneyCheck {
        name: name.into(),
        passed,
        expected: expected.into(),
        actual: actual.into(),
        critical,
    }
}

fn number_check(
    bundle: &crate::rehab_context::RehabCaseBundle,
    key: &str,
    expected: f64,
    critical: bool,
) -> JourneyCheck {
    let actual = bundle
        .cohort_row
        .derived_values
        .get(key)
        .and_then(Value::as_f64);
    check(
        key,
        actual == Some(expected),
        &expected.to_string(),
        &actual
            .map(|value| value.to_string())
            .unwrap_or_else(|| "missing".into()),
        critical,
    )
}

fn metric(id: &str, label: &str, value: f64, threshold: f64, unit: &str) -> EvalMetric {
    EvalMetric {
        id: id.into(),
        label: label.into(),
        value,
        threshold,
        passed: value >= threshold,
        unit: unit.into(),
    }
}

fn build_recommendations(journeys: &[GoldenJourneyResult], metrics: &[EvalMetric]) -> Vec<String> {
    let mut output = Vec::new();
    for journey in journeys.iter().filter(|journey| !journey.passed) {
        let failed = journey
            .checks
            .iter()
            .filter(|check| !check.passed)
            .map(|check| check.name.as_str())
            .collect::<Vec<_>>()
            .join("、");
        output.push(format!(
            "优先修复 {}（{}）：{}",
            journey.journey_id, journey.title, failed
        ));
    }
    for metric in metrics.iter().filter(|metric| !metric.passed) {
        output.push(format!(
            "{} 为 {:.1}%，低于门槛 {:.1}%",
            metric.label,
            metric.value * 100.0,
            metric.threshold * 100.0
        ));
    }
    if output.is_empty() {
        output.push(
            "当前确定性黄金旅程全部通过；下一步加入 OCR 噪声、模型超时和重复点击扰动。".into(),
        );
    }
    output
}

fn nonce() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
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

    #[test]
    fn metric_threshold_is_inclusive() {
        assert!(metric("x", "x", 0.95, 0.95, "ratio").passed);
        assert!(!metric("x", "x", 0.94, 0.95, "ratio").passed);
    }

    #[test]
    fn failed_journey_produces_actionable_recommendation() {
        let result = journey(
            "JX",
            "测试",
            "用户",
            Instant::now(),
            vec![],
            vec![check("区域配对", false, "胸弯", "腰弯", true)],
        );
        let recommendations = build_recommendations(&[result], &[]);
        assert!(recommendations[0].contains("区域配对"));
    }
}
