use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClinicalCaseInput {
    pub case_text: String,
    #[serde(default)]
    pub age: Option<u8>,
    #[serde(default)]
    pub sex: Option<String>,
    #[serde(default)]
    pub context: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FindingPolarity {
    Present,
    Absent,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClinicalFinding {
    pub term: String,
    pub category: String,
    pub polarity: FindingPolarity,
    pub evidence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskSignal {
    pub signal: String,
    pub level: RiskLevel,
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateDisease {
    pub disease: String,
    pub system: String,
    pub priority_score: u8,
    pub support_evidence: Vec<String>,
    pub contrary_evidence: Vec<String>,
    pub missing_information: Vec<String>,
    pub reasoning_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClinicalReasoningReport {
    pub normalized_summary: String,
    pub structured_findings: Vec<ClinicalFinding>,
    pub risk_signals: Vec<RiskSignal>,
    pub candidate_diseases: Vec<CandidateDisease>,
    pub information_gaps: Vec<String>,
    pub teaching_note: String,
    pub safety_boundary: String,
}

#[derive(Debug, Clone)]
struct SymptomRule {
    term: &'static str,
    category: &'static str,
    keywords: &'static [&'static str],
}

#[derive(Debug, Clone)]
struct DiseaseRule {
    disease: &'static str,
    system: &'static str,
    support_terms: &'static [&'static str],
    contrary_terms: &'static [&'static str],
    missing_information: &'static [&'static str],
}

const SYMPTOM_RULES: &[SymptomRule] = &[
    SymptomRule {
        term: "胸痛/胸闷",
        category: "主诉症状",
        keywords: &[
            "胸痛",
            "胸闷",
            "胸口痛",
            "胸口闷",
            "chest pain",
            "chest tightness",
        ],
    },
    SymptomRule {
        term: "活动后加重",
        category: "诱因与加重因素",
        keywords: &["活动后", "劳力", "运动后", "exertional", "on exertion"],
    },
    SymptomRule {
        term: "气促/呼吸困难",
        category: "呼吸循环症状",
        keywords: &[
            "气促",
            "喘",
            "呼吸困难",
            "憋气",
            "dyspnea",
            "shortness of breath",
        ],
    },
    SymptomRule {
        term: "发热",
        category: "全身症状",
        keywords: &["发热", "发烧", "高热", "fever", "febrile"],
    },
    SymptomRule {
        term: "咳嗽",
        category: "呼吸道症状",
        keywords: &["咳嗽", "干咳", "cough"],
    },
    SymptomRule {
        term: "咳痰",
        category: "呼吸道症状",
        keywords: &["咳痰", "黄痰", "白痰", "sputum", "phlegm"],
    },
    SymptomRule {
        term: "心悸",
        category: "循环系统症状",
        keywords: &["心悸", "心慌", "palpitation", "palpitations"],
    },
    SymptomRule {
        term: "晕厥/意识改变",
        category: "危险信号",
        keywords: &[
            "晕厥",
            "昏厥",
            "意识障碍",
            "意识改变",
            "syncope",
            "loss of consciousness",
        ],
    },
    SymptomRule {
        term: "下肢水肿",
        category: "循环系统症状",
        keywords: &["下肢水肿", "腿肿", "水肿", "leg swelling", "edema"],
    },
    SymptomRule {
        term: "咯血",
        category: "危险信号",
        keywords: &["咯血", "咳血", "hemoptysis"],
    },
    SymptomRule {
        term: "腹痛",
        category: "消化系统症状",
        keywords: &["腹痛", "肚子痛", "abdominal pain"],
    },
    SymptomRule {
        term: "恶心/呕吐",
        category: "消化系统症状",
        keywords: &["恶心", "呕吐", "nausea", "vomiting"],
    },
    SymptomRule {
        term: "头痛",
        category: "神经系统症状",
        keywords: &["头痛", "headache"],
    },
    SymptomRule {
        term: "单侧肢体无力",
        category: "危险信号",
        keywords: &["偏瘫", "单侧无力", "肢体无力", "unilateral weakness"],
    },
];

const DISEASE_RULES: &[DiseaseRule] = &[
    DiseaseRule {
        disease: "急性冠脉综合征",
        system: "心血管系统",
        support_terms: &["胸痛/胸闷", "活动后加重", "气促/呼吸困难", "心悸"],
        contrary_terms: &["发热", "咳嗽", "咳痰"],
        missing_information: &[
            "心电图",
            "肌钙蛋白",
            "既往冠心病或危险因素",
            "疼痛性质与持续时间",
        ],
    },
    DiseaseRule {
        disease: "肺部感染/肺炎",
        system: "呼吸系统",
        support_terms: &["发热", "咳嗽", "咳痰", "气促/呼吸困难"],
        contrary_terms: &["胸痛/胸闷"],
        missing_information: &["体温", "血常规及炎症指标", "胸部影像", "血氧饱和度"],
    },
    DiseaseRule {
        disease: "哮喘或慢阻肺急性加重",
        system: "呼吸系统",
        support_terms: &["气促/呼吸困难", "咳嗽", "活动后加重"],
        contrary_terms: &["咯血", "下肢水肿"],
        missing_information: &[
            "喘鸣音",
            "既往哮喘或慢阻肺病史",
            "肺功能或峰流速",
            "诱发因素",
        ],
    },
    DiseaseRule {
        disease: "心力衰竭",
        system: "心血管系统",
        support_terms: &["气促/呼吸困难", "活动后加重", "下肢水肿", "心悸"],
        contrary_terms: &["发热", "咳痰"],
        missing_information: &[
            "BNP/NT-proBNP",
            "心脏超声",
            "夜间阵发性呼吸困难",
            "既往心脏病史",
        ],
    },
    DiseaseRule {
        disease: "肺栓塞",
        system: "呼吸循环系统",
        support_terms: &[
            "胸痛/胸闷",
            "气促/呼吸困难",
            "咯血",
            "下肢水肿",
            "晕厥/意识改变",
        ],
        contrary_terms: &["发热", "咳痰"],
        missing_information: &[
            "D-二聚体",
            "下肢静脉血栓风险",
            "CTA 或肺动脉成像",
            "近期手术/制动史",
        ],
    },
    DiseaseRule {
        disease: "胃食管反流或消化系统相关胸痛",
        system: "消化系统",
        support_terms: &["胸痛/胸闷", "腹痛", "恶心/呕吐"],
        contrary_terms: &["晕厥/意识改变", "气促/呼吸困难"],
        missing_information: &["进食相关性", "反酸烧心", "腹部查体", "心源性胸痛排除依据"],
    },
    DiseaseRule {
        disease: "脑血管事件",
        system: "神经系统",
        support_terms: &["头痛", "单侧肢体无力", "晕厥/意识改变"],
        contrary_terms: &["咳嗽", "咳痰"],
        missing_information: &[
            "神经系统查体",
            "发病时间窗",
            "头颅 CT/MRI",
            "血压与凝血状态",
        ],
    },
];

/// Unified entry point: validate input, run analysis, format output.
/// Used by both the Tauri command and the model tool — keeps them in sync.
pub fn run(input: ClinicalCaseInput, output_format: &str) -> Result<String, String> {
    if input.case_text.trim().is_empty() {
        return Err("case_text cannot be empty".into());
    }
    let report = analyze_case(&input);
    match output_format {
        "markdown" => Ok(format_report(&report)),
        "json" => serde_json::to_string_pretty(&report)
            .map_err(|e| format!("Failed to serialize clinical report: {e}")),
        other => Err(format!("Unsupported output_format: {other}")),
    }
}

pub fn analyze_case(input: &ClinicalCaseInput) -> ClinicalReasoningReport {
    let findings = extract_findings(&input.case_text);
    let risk_signals = detect_risk_signals(&findings);
    let mut candidates = rank_candidates(&findings);
    candidates.sort_by(|a, b| {
        b.priority_score
            .cmp(&a.priority_score)
            .then_with(|| a.disease.cmp(&b.disease))
    });

    ClinicalReasoningReport {
        normalized_summary: build_summary(input, &findings),
        information_gaps: collect_information_gaps(&candidates),
        structured_findings: findings,
        risk_signals,
        candidate_diseases: candidates,
        teaching_note: "该报告用于展示临床推理训练过程，重点呈现症状结构化、候选诊断生成、证据支持与排除依据。".to_string(),
        safety_boundary: "本结果仅用于医学教学、科研训练和临床思维演示，不构成临床诊断、治疗决策或医疗处置依据。存在急危重症风险时应及时由专业医务人员评估。".to_string(),
    }
}

pub fn format_report(report: &ClinicalReasoningReport) -> String {
    let mut out = String::new();
    out.push_str("# Galen-MedX 临床推理报告\n\n");
    out.push_str("## 病例结构化摘要\n");
    out.push_str(&report.normalized_summary);
    out.push_str("\n\n## 症状结构化结果\n");
    for finding in &report.structured_findings {
        let polarity = match finding.polarity {
            FindingPolarity::Present => "阳性",
            FindingPolarity::Absent => "阴性",
            FindingPolarity::Unknown => "未明确",
        };
        out.push_str(&format!(
            "- [{}] {}：{}（证据：{}）\n",
            polarity, finding.category, finding.term, finding.evidence
        ));
    }
    if report.structured_findings.is_empty() {
        out.push_str("- 未识别到足够明确的症状信息。\n");
    }

    out.push_str("\n## 风险识别\n");
    if report.risk_signals.is_empty() {
        out.push_str("- 未识别到明确急危重症危险信号；仍需结合生命体征和查体结果判断。\n");
    } else {
        for signal in &report.risk_signals {
            out.push_str(&format!(
                "- {:?}：{}。{}\n",
                signal.level, signal.signal, signal.rationale
            ));
        }
    }

    out.push_str("\n## 候选疾病与排除推理\n");
    for (index, candidate) in report.candidate_diseases.iter().enumerate() {
        out.push_str(&format!(
            "{}. {}（{}，评分 {}，{}）\n",
            index + 1,
            candidate.disease,
            candidate.system,
            candidate.priority_score,
            candidate.reasoning_status
        ));
        out.push_str(&format!(
            "   - 支持证据：{}\n",
            format_list(&candidate.support_evidence)
        ));
        out.push_str(&format!(
            "   - 反向证据：{}\n",
            format_list(&candidate.contrary_evidence)
        ));
        out.push_str(&format!(
            "   - 信息缺口：{}\n",
            format_list(&candidate.missing_information)
        ));
    }

    out.push_str("\n## 信息缺口清单\n");
    for gap in &report.information_gaps {
        out.push_str(&format!("- {gap}\n"));
    }

    out.push_str("\n## 教学提示\n");
    out.push_str(&report.teaching_note);
    out.push_str("\n\n## 医学安全边界\n");
    out.push_str(&report.safety_boundary);
    out
}

fn extract_findings(case_text: &str) -> Vec<ClinicalFinding> {
    let lower = case_text.to_lowercase();
    let mut findings = Vec::new();

    for rule in SYMPTOM_RULES {
        if let Some(keyword) = rule
            .keywords
            .iter()
            .find(|keyword| lower.contains(&keyword.to_lowercase()))
        {
            let polarity = if is_negated(&lower, keyword) {
                FindingPolarity::Absent
            } else {
                FindingPolarity::Present
            };
            findings.push(ClinicalFinding {
                term: rule.term.to_string(),
                category: rule.category.to_string(),
                polarity,
                evidence: (*keyword).to_string(),
            });
        }
    }

    findings
}

fn is_negated(text: &str, keyword: &str) -> bool {
    let keyword_lower = keyword.to_lowercase();
    let chinese_negations = ["无", "没有", "否认", "未见", "不伴", "无明显"];
    let english_negations = ["no ", "denies ", "without ", "negative for "];

    chinese_negations
        .iter()
        .any(|prefix| text.contains(&format!("{prefix}{keyword_lower}")))
        || english_negations
            .iter()
            .any(|prefix| text.contains(&format!("{prefix}{keyword_lower}")))
}

fn detect_risk_signals(findings: &[ClinicalFinding]) -> Vec<RiskSignal> {
    let present = |term: &str| has_present(findings, term);
    let mut signals = Vec::new();

    if present("胸痛/胸闷") && present("气促/呼吸困难") {
        signals.push(RiskSignal {
            signal: "胸痛/胸闷合并气促".to_string(),
            level: RiskLevel::High,
            rationale: "该组合需要优先排查急性冠脉综合征、肺栓塞等高风险疾病。".to_string(),
        });
    }
    if present("晕厥/意识改变") {
        signals.push(RiskSignal {
            signal: "晕厥或意识改变".to_string(),
            level: RiskLevel::High,
            rationale: "意识改变提示潜在循环、神经或代谢急症，需优先评估生命体征。".to_string(),
        });
    }
    if present("咯血") && present("气促/呼吸困难") {
        signals.push(RiskSignal {
            signal: "咯血合并呼吸困难".to_string(),
            level: RiskLevel::High,
            rationale: "需考虑肺栓塞、严重感染或其他呼吸系统急症。".to_string(),
        });
    }
    if present("单侧肢体无力") {
        signals.push(RiskSignal {
            signal: "局灶神经功能缺损".to_string(),
            level: RiskLevel::High,
            rationale: "单侧肢体无力需优先排查脑血管事件。".to_string(),
        });
    }
    if present("发热") && present("气促/呼吸困难") {
        signals.push(RiskSignal {
            signal: "发热合并呼吸困难".to_string(),
            level: RiskLevel::Medium,
            rationale: "提示感染相关呼吸系统疾病风险，需要结合血氧和影像判断。".to_string(),
        });
    }

    signals
}

fn rank_candidates(findings: &[ClinicalFinding]) -> Vec<CandidateDisease> {
    DISEASE_RULES
        .iter()
        .map(|rule| {
            let mut support_evidence = Vec::new();
            let mut contrary_evidence = Vec::new();
            let mut score = 0_u8;

            for term in rule.support_terms {
                if has_present(findings, term) {
                    score = score.saturating_add(20);
                    support_evidence.push((*term).to_string());
                }
                if has_absent(findings, term) {
                    contrary_evidence.push(format!("缺乏 {}", term));
                }
            }

            for term in rule.contrary_terms {
                if has_present(findings, term) {
                    score = score.saturating_sub(8);
                    contrary_evidence.push((*term).to_string());
                }
                if has_absent(findings, term) {
                    support_evidence.push(format!("阴性证据：{}", term));
                    score = score.saturating_add(4);
                }
            }

            if score > 100 {
                score = 100;
            }

            CandidateDisease {
                disease: rule.disease.to_string(),
                system: rule.system.to_string(),
                priority_score: score,
                support_evidence,
                contrary_evidence,
                missing_information: rule
                    .missing_information
                    .iter()
                    .map(|item| (*item).to_string())
                    .collect(),
                reasoning_status: status_from_score(score).to_string(),
            }
        })
        .filter(|candidate| candidate.priority_score > 0 || !candidate.contrary_evidence.is_empty())
        .collect()
}

fn status_from_score(score: u8) -> &'static str {
    match score {
        70..=100 => "优先考虑",
        40..=69 => "需要鉴别",
        1..=39 => "可能性较低但需结合证据",
        _ => "证据不足",
    }
}

fn has_present(findings: &[ClinicalFinding], term: &str) -> bool {
    findings
        .iter()
        .any(|finding| finding.term == term && finding.polarity == FindingPolarity::Present)
}

fn has_absent(findings: &[ClinicalFinding], term: &str) -> bool {
    findings
        .iter()
        .any(|finding| finding.term == term && finding.polarity == FindingPolarity::Absent)
}

fn build_summary(input: &ClinicalCaseInput, findings: &[ClinicalFinding]) -> String {
    let age = input
        .age
        .map(|value| format!("{value}岁"))
        .unwrap_or_else(|| "年龄未提供".to_string());
    let sex = input.sex.as_deref().unwrap_or("性别未提供");
    let positive_terms = findings
        .iter()
        .filter(|finding| finding.polarity == FindingPolarity::Present)
        .map(|finding| finding.term.as_str())
        .collect::<Vec<_>>();
    let absent_terms = findings
        .iter()
        .filter(|finding| finding.polarity == FindingPolarity::Absent)
        .map(|finding| finding.term.as_str())
        .collect::<Vec<_>>();

    format!(
        "患者基本信息：{}，{}。阳性信息：{}。阴性信息：{}。原始描述：{}",
        age,
        sex,
        if positive_terms.is_empty() {
            "暂未识别到明确阳性症状".to_string()
        } else {
            positive_terms.join("、")
        },
        if absent_terms.is_empty() {
            "暂未识别到明确阴性症状".to_string()
        } else {
            absent_terms.join("、")
        },
        input.case_text
    )
}

fn collect_information_gaps(candidates: &[CandidateDisease]) -> Vec<String> {
    let mut gaps = Vec::new();
    for candidate in candidates.iter().take(4) {
        for item in &candidate.missing_information {
            if !gaps.contains(item) {
                gaps.push(item.clone());
            }
        }
    }
    gaps
}

fn format_list(items: &[String]) -> String {
    if items.is_empty() {
        "无明确记录".to_string()
    } else {
        items.join("、")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_validates_empty_input() {
        let result = run(
            ClinicalCaseInput {
                case_text: "  ".into(),
                age: None,
                sex: None,
                context: None,
            },
            "markdown",
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("cannot be empty"));
    }

    #[test]
    fn run_rejects_unknown_format() {
        let result = run(
            ClinicalCaseInput {
                case_text: "chest pain".into(),
                age: None,
                sex: None,
                context: None,
            },
            "xml",
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unsupported output_format"));
    }

    #[test]
    fn run_produces_markdown_and_json() {
        let input = ClinicalCaseInput {
            case_text: "胸痛、气促".into(),
            age: Some(45),
            sex: Some("男".into()),
            context: None,
        };
        let md = run(input.clone(), "markdown").expect("markdown");
        assert!(md.contains("Galen-MedX"));
        assert!(md.contains("医学安全边界"));

        let json = run(input, "json").expect("json");
        assert!(json.contains("normalized_summary"));
        assert!(json.contains("risk_signals"));
    }

    #[test]
    fn extracts_positive_and_negative_findings() {
        let input = ClinicalCaseInput {
            case_text: "胸口闷，活动后明显，有点喘，无发热，无咳痰。".to_string(),
            age: Some(62),
            sex: Some("男".to_string()),
            context: None,
        };
        let report = analyze_case(&input);
        assert!(report
            .structured_findings
            .iter()
            .any(|finding| finding.term == "胸痛/胸闷"
                && finding.polarity == FindingPolarity::Present));
        assert!(report
            .structured_findings
            .iter()
            .any(|finding| finding.term == "发热" && finding.polarity == FindingPolarity::Absent));
        assert_eq!(report.candidate_diseases[0].disease, "急性冠脉综合征");
    }

    #[test]
    fn flags_high_risk_chest_symptoms() {
        let input = ClinicalCaseInput {
            case_text: "突发胸痛伴呼吸困难，活动后加重。".to_string(),
            age: None,
            sex: None,
            context: None,
        };
        let report = analyze_case(&input);
        assert!(report
            .risk_signals
            .iter()
            .any(|signal| signal.level == RiskLevel::High));
    }

    #[test]
    fn formats_report_with_safety_boundary() {
        let input = ClinicalCaseInput {
            case_text: "发热、咳嗽、咳痰三天，伴气促。".to_string(),
            age: None,
            sex: None,
            context: None,
        };
        let report = analyze_case(&input);
        let formatted = format_report(&report);
        assert!(formatted.contains("Galen-MedX 临床推理报告"));
        assert!(formatted.contains("医学安全边界"));
        assert!(formatted.contains("肺部感染"));
    }
}
