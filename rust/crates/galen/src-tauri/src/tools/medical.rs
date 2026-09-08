use api::ToolDefinition;
use async_trait::async_trait;
use medical_core::rehab::{build_rehab_query, RehabFocus, RehabStudyType};
use medical_core::types::CitationStyle;
use medical_core::types::Paper;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

use super::{GalenTool, ToolContext, ToolExecution};
use crate::backend::ChatEvent;

// ---------------------------------------------------------------------------
// SearchPubMed
// ---------------------------------------------------------------------------

pub struct SearchPubMed;

#[async_trait]
impl GalenTool for SearchPubMed {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "search_pubmed".into(),
            description: Some(
                "Search PubMed for medical literature. Returns papers with PMID, title, authors, journal, year, DOI.".into(),
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "PubMed search query"},
                    "research_question": {"type": "string", "description": "Optional plain-language question used to rank candidates by topic fit"},
                    "max_results": {"type": "integer", "description": "Max results (1-20, default 10)"}
                },
                "required": ["query"]
            }),
        }
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String, String> {
        execute_pubmed(input, ctx).await.result
    }

    async fn execute_observed(&self, input: Value, ctx: &ToolContext) -> ToolExecution {
        execute_pubmed(input, ctx).await
    }
}

async fn execute_pubmed(input: Value, ctx: &ToolContext) -> ToolExecution {
    let Some(query) = input["query"].as_str() else {
        return ToolExecution::from_result(Err("Missing 'query' parameter".into()));
    };
    let limit = input["max_results"].as_u64().unwrap_or(10).clamp(1, 20) as u32;
    let research_question = input["research_question"]
        .as_str()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(query);
    let candidates = match ctx
        .medical
        .search_pubmed(query, candidate_limit(limit))
        .await
    {
        Ok(papers) => papers,
        Err(error) => {
            return ToolExecution::from_result(Err(format!("PubMed search error: {error}")))
        }
    };
    let ranked = rank_papers(&candidates, research_question, limit as usize);
    let papers = ranked
        .iter()
        .map(|item| item.paper.clone())
        .collect::<Vec<_>>();
    ctx.send_event(ChatEvent::SearchResults(papers.clone()));
    let text = if ranked.is_empty() {
        "No results found.".into()
    } else {
        format_search_report(query, research_question, candidates.len(), &ranked)
    };
    ToolExecution {
        result: Ok(text),
        raw_output: serde_json::to_value(&papers).ok(),
        result_count: Some(papers.len()),
        query: Some(query.to_string()),
    }
}

// ---------------------------------------------------------------------------
// FetchArticle
// ---------------------------------------------------------------------------

pub struct FetchArticle;

#[async_trait]
impl GalenTool for FetchArticle {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "fetch_article".into(),
            description: Some("Fetch detailed metadata for a PubMed article by PMID.".into()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pmid": {"type": "string", "description": "PubMed ID"}
                },
                "required": ["pmid"]
            }),
        }
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String, String> {
        let pmid = input["pmid"].as_str().ok_or("Missing 'pmid'")?;
        let paper = ctx
            .medical
            .fetch_article(pmid)
            .await
            .map_err(|e| format!("PubMed fetch error: {e}"))?;
        match paper {
            None => Ok(format!("No article found for PMID: {pmid}")),
            Some(p) => Ok(format!(
                "Title: {}\nAuthors: {}\nJournal: {}\nYear: {}\nDOI: {}\n\nAbstract:\n{}",
                p.title,
                p.authors
                    .iter()
                    .map(|a| a.to_string())
                    .collect::<Vec<_>>()
                    .join(", "),
                p.journal.as_deref().unwrap_or("N/A"),
                p.year.as_deref().unwrap_or("N/A"),
                p.doi.as_deref().unwrap_or("N/A"),
                p.abstract_text.as_deref().unwrap_or("No abstract"),
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// VerifyCitation
// ---------------------------------------------------------------------------

/// Resolve a cited PMID against PubMed before it can be treated as evidence.
/// This deliberately verifies database metadata, not a model-generated string.
pub struct VerifyCitation;

#[async_trait]
impl GalenTool for VerifyCitation {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "verify_citation".into(),
            description: Some(
                "Verify a PMID against the live PubMed record. Optionally compares a claimed title or DOI and returns verified, partial, or rejected with a metadata fingerprint. Use before citing a source that did not originate from Galen search results."
                    .into(),
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pmid": {"type": "string", "description": "PubMed identifier to verify"},
                    "expected_title": {"type": "string", "description": "Optional claimed title to compare"},
                    "expected_doi": {"type": "string", "description": "Optional claimed DOI to compare"}
                },
                "required": ["pmid"]
            }),
        }
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String, String> {
        let pmid = input["pmid"].as_str().ok_or("Missing 'pmid'")?;
        let expected_title = input["expected_title"].as_str();
        let expected_doi = input["expected_doi"].as_str();
        let paper = ctx
            .medical
            .fetch_article(pmid)
            .await
            .map_err(|error| format!("PubMed verification error: {error}"))?
            .ok_or_else(|| format!("PubMed 未找到 PMID: {pmid}，该引用不能作为证据使用。"))?;
        Ok(format_verification(&verify_pubmed_paper(
            &paper,
            expected_title,
            expected_doi,
        )))
    }
}

// ---------------------------------------------------------------------------
// FormatCitation
// ---------------------------------------------------------------------------

pub struct FormatCitation;

#[async_trait]
impl GalenTool for FormatCitation {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "format_citation".into(),
            description: Some(
                "Format papers into a citation style (apa, vancouver, bibtex, ris, mla).".into(),
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pmids": {"type": "array", "items": {"type": "string"}},
                    "style": {"type": "string", "enum": ["apa", "vancouver", "bibtex", "ris", "mla"]}
                },
                "required": ["pmids", "style"]
            }),
        }
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String, String> {
        let pmids: Vec<String> = input["pmids"]
            .as_array()
            .ok_or("Missing 'pmids'")?
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();
        let style = CitationStyle::from_str(input["style"].as_str().ok_or("Missing 'style'")?)
            .ok_or("Unknown citation style")?;
        let papers = ctx
            .medical
            .pubmed
            .fetch_articles(&pmids)
            .await
            .map_err(|e| format!("PubMed fetch error: {e}"))?;
        Ok(ctx.medical.format_citations(&papers, style))
    }
}

// ---------------------------------------------------------------------------
// SearchRehabLiterature
// ---------------------------------------------------------------------------

pub struct SearchRehabLiterature;

#[async_trait]
impl GalenTool for SearchRehabLiterature {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "search_rehab_literature".into(),
            description: Some(
                "Search PubMed for rehabilitation-specific literature. Adds rehabilitation MeSH terms and optional study-design filters (RCT / systematic review / meta-analysis).".into(),
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "topic": {"type": "string", "description": "Rehabilitation topic in plain language, e.g. 'trunk control after stroke'"},
                    "focus": {"type": "string", "enum": ["neuro", "ortho", "pediatric", "cardiopulmonary", "spinal"], "description": "Optional sub-specialty focus"},
                    "study_type": {"type": "string", "enum": ["any", "rct", "systematic_review", "meta_analysis"], "description": "Evidence-level filter (default: rct)"},
                    "max_results": {"type": "integer", "description": "Max results (1-20, default 10)"}
                },
                "required": ["topic"]
            }),
        }
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String, String> {
        execute_rehab_search(input, ctx).await.result
    }

    async fn execute_observed(&self, input: Value, ctx: &ToolContext) -> ToolExecution {
        execute_rehab_search(input, ctx).await
    }
}

async fn execute_rehab_search(input: Value, ctx: &ToolContext) -> ToolExecution {
    let Some(topic) = input["topic"].as_str() else {
        return ToolExecution::from_result(Err("Missing 'topic' parameter".into()));
    };
    let focus = input["focus"].as_str().and_then(RehabFocus::from_id);
    let study_type = RehabStudyType::from_id(input["study_type"].as_str().unwrap_or("rct"));
    let limit = input["max_results"].as_u64().unwrap_or(10).clamp(1, 20) as u32;
    let query = build_rehab_query(topic, focus, study_type);
    let candidates = match ctx
        .medical
        .search_pubmed(&query, candidate_limit(limit))
        .await
    {
        Ok(papers) => papers,
        Err(error) => {
            return ToolExecution::from_result(Err(format!("PubMed search error: {error}")))
        }
    };
    let ranked = rank_papers(&candidates, topic, limit as usize);
    let papers = ranked
        .iter()
        .map(|item| item.paper.clone())
        .collect::<Vec<_>>();
    ctx.send_event(ChatEvent::SearchResults(papers.clone()));
    let text = if ranked.is_empty() {
        format!("No results found.\nQuery used: {query}")
    } else {
        format_search_report(&query, topic, candidates.len(), &ranked)
    };
    ToolExecution {
        result: Ok(text),
        raw_output: serde_json::to_value(&papers).ok(),
        result_count: Some(papers.len()),
        query: Some(query),
    }
}

// ---------------------------------------------------------------------------
// Search quality gate and shared formatting
// ---------------------------------------------------------------------------

struct RankedPaper<'a> {
    paper: &'a Paper,
    score: u8,
    matched_terms: Vec<String>,
    verification: CitationVerification,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VerificationStatus {
    Verified,
    Partial,
    Rejected,
}

impl VerificationStatus {
    fn label(self) -> &'static str {
        match self {
            Self::Verified => "已验证 · PubMed",
            Self::Partial => "部分验证 · 待补全",
            Self::Rejected => "核验失败 · 禁止引用",
        }
    }
}

#[derive(Debug, Clone)]
struct CitationVerification {
    status: VerificationStatus,
    paper: Paper,
    checks: Vec<String>,
    fingerprint: String,
}

fn verify_pubmed_paper(
    paper: &Paper,
    expected_title: Option<&str>,
    expected_doi: Option<&str>,
) -> CitationVerification {
    let mut checks = Vec::new();
    let mut status = VerificationStatus::Verified;
    if paper.pmid.len() < 5
        || paper.pmid.len() > 9
        || !paper
            .pmid
            .chars()
            .all(|character| character.is_ascii_digit())
    {
        status = VerificationStatus::Rejected;
        checks.push("PMID 格式无效".to_string());
    } else {
        checks.push("PMID 已由 PubMed 解析".to_string());
    }
    if paper.title.trim().is_empty() {
        status = VerificationStatus::Rejected;
        checks.push("PubMed 记录缺少标题".to_string());
    } else {
        checks.push("标题已取得".to_string());
    }
    if paper.journal.as_deref().is_none_or(str::is_empty)
        || paper.year.as_deref().is_none_or(str::is_empty)
    {
        if status != VerificationStatus::Rejected {
            status = VerificationStatus::Partial;
        }
        checks.push("期刊或年份缺失".to_string());
    } else {
        checks.push("期刊与年份已取得".to_string());
    }
    if let Some(title) = expected_title.filter(|title| !title.trim().is_empty()) {
        if title_matches(title, &paper.title) {
            checks.push("声明标题与 PubMed 记录匹配".to_string());
        } else {
            status = VerificationStatus::Rejected;
            checks.push("声明标题与 PubMed 记录冲突".to_string());
        }
    }
    if let Some(doi) = expected_doi.filter(|doi| !doi.trim().is_empty()) {
        match paper.doi.as_deref() {
            Some(actual) if normalize_doi(actual) == normalize_doi(doi) => {
                checks.push("声明 DOI 与 PubMed 记录匹配".to_string());
            }
            Some(_) => {
                status = VerificationStatus::Rejected;
                checks.push("声明 DOI 与 PubMed 记录冲突".to_string());
            }
            None => {
                if status != VerificationStatus::Rejected {
                    status = VerificationStatus::Partial;
                }
                checks.push("PubMed 记录未提供 DOI，需到 DOI 注册机构补核".to_string());
            }
        }
    }
    CitationVerification {
        status,
        paper: paper.clone(),
        checks,
        fingerprint: metadata_fingerprint(paper),
    }
}

fn title_matches(expected: &str, actual: &str) -> bool {
    let expected = normalized_words(expected);
    let actual = normalized_words(actual);
    if expected.is_empty() || actual.is_empty() {
        return false;
    }
    let intersection = expected.intersection(&actual).count();
    intersection * 100 / expected.len() >= 70
}

fn normalized_words(value: &str) -> BTreeSet<String> {
    normalize(value)
        .split_whitespace()
        .filter(|word| word.len() > 2)
        .map(ToString::to_string)
        .collect()
}

fn normalize_doi(doi: &str) -> String {
    doi.trim()
        .trim_start_matches("https://doi.org/")
        .trim_start_matches("http://doi.org/")
        .trim_end_matches(['.', ',', ';', ')'])
        .to_ascii_lowercase()
}

fn metadata_fingerprint(paper: &Paper) -> String {
    let content = format!(
        "{}|{}|{}|{}|{}",
        paper.pmid,
        normalize(&paper.title),
        paper.journal.as_deref().unwrap_or_default(),
        paper.year.as_deref().unwrap_or_default(),
        paper.doi.as_deref().unwrap_or_default(),
    );
    format!("{:x}", Sha256::digest(content.as_bytes()))[..16].to_string()
}

fn format_verification(verification: &CitationVerification) -> String {
    let paper = &verification.paper;
    format!(
        "状态：{}\n[PMID: {}](https://pubmed.ncbi.nlm.nih.gov/{}/)\n标题：{}\n期刊：{}\n年份：{}\nDOI：{}\n元数据指纹：{}\n核验项：{}",
        verification.status.label(),
        paper.pmid,
        paper.pmid,
        paper.title,
        paper.journal.as_deref().unwrap_or("未提供"),
        paper.year.as_deref().unwrap_or("未提供"),
        paper.doi.as_deref().unwrap_or("未提供"),
        verification.fingerprint,
        verification.checks.join("；"),
    )
}

fn candidate_limit(limit: u32) -> u32 {
    limit.saturating_mul(3).clamp(10, 60)
}

fn rank_papers<'a>(
    papers: &'a [Paper],
    research_question: &str,
    limit: usize,
) -> Vec<RankedPaper<'a>> {
    let terms = search_terms(research_question);
    let mut ranked = papers
        .iter()
        .map(|paper| {
            let title = normalize(&paper.title);
            let abstract_text = normalize(paper.abstract_text.as_deref().unwrap_or_default());
            let mesh = normalize(
                &paper
                    .mesh_terms
                    .iter()
                    .map(|term| term.descriptor.as_str())
                    .collect::<Vec<_>>()
                    .join(" "),
            );
            let mut matched_terms = Vec::new();
            let mut weighted_hits = 0_u32;
            for term in &terms {
                let title_hit = title.contains(term);
                let abstract_hit = abstract_text.contains(term);
                let mesh_hit = mesh.contains(term);
                if title_hit || abstract_hit || mesh_hit {
                    matched_terms.push(term.clone());
                    weighted_hits += if title_hit { 5 } else { 0 };
                    weighted_hits += if mesh_hit { 3 } else { 0 };
                    weighted_hits += if abstract_hit { 2 } else { 0 };
                }
            }
            let coverage = if terms.is_empty() {
                0
            } else {
                (matched_terms.len() as u32 * 70 / terms.len() as u32) as u8
            };
            RankedPaper {
                paper,
                score: coverage
                    .saturating_add(weighted_hits.min(30) as u8)
                    .min(100),
                matched_terms,
                verification: verify_pubmed_paper(paper, None, None),
            }
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| right.paper.year.cmp(&left.paper.year))
            .then_with(|| left.paper.pmid.cmp(&right.paper.pmid))
    });
    // A record without a valid PubMed identity is not a literature result;
    // never surface it as a candidate the model could later cite.
    ranked.retain(|item| item.verification.status != VerificationStatus::Rejected);
    ranked.truncate(limit);
    ranked
}

fn search_terms(research_question: &str) -> Vec<String> {
    const STOP_WORDS: &[&str] = &[
        "and",
        "or",
        "not",
        "the",
        "with",
        "for",
        "from",
        "after",
        "before",
        "mesh",
        "title",
        "abstract",
        "publication",
        "type",
        "all",
        "fields",
        "rehabilitation",
    ];
    let normalized = normalize(research_question);
    let mut terms = normalized
        .split_whitespace()
        .filter(|term| term.len() >= 3 && !STOP_WORDS.contains(term))
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    for (needle, aliases) in [
        (
            "t2dm",
            &["diabetes", "type 2 diabetes", "type ii diabetes"][..],
        ),
        (
            "diabetes",
            &["t2dm", "type 2 diabetes", "type ii diabetes"][..],
        ),
        ("stroke", &["cerebrovascular accident", "cva"][..]),
        ("upper limb", &["upper extremity", "arm"][..]),
        ("upper extremity", &["upper limb", "arm"][..]),
        ("prognosis", &["prognostic", "predictor", "prediction"][..]),
        ("risk", &["predictor", "prediction"][..]),
        ("adherence", &["compliance", "engagement"][..]),
    ] {
        if normalized.contains(needle) {
            terms.extend(aliases.iter().map(|alias| alias.to_string()));
        }
    }
    terms.sort();
    terms.dedup();
    terms
}

fn normalize(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn format_search_report(
    query: &str,
    research_question: &str,
    candidate_count: usize,
    ranked: &[RankedPaper<'_>],
) -> String {
    let low_fit_count = ranked.iter().filter(|item| item.score < 45).count();
    let mut lines = vec![
        format!("检索式：{query}"),
        format!("问题锚点：{research_question}"),
        format!("来源覆盖：本轮仅 PubMed；检出 {candidate_count} 篇候选，按问题匹配度筛选并展示前 {} 篇。", ranked.len()),
    ];
    if low_fit_count > 0 {
        lines.push(format!(
            "质量提醒：当前展示中有 {low_fit_count} 篇匹配度偏低，不能直接作为结论依据；应补充同义词或改写检索式后复检。"
        ));
    }
    for item in ranked {
        let paper = item.paper;
        let authors = if paper.authors.is_empty() {
            "Unknown".to_string()
        } else if paper.authors.len() == 1 {
            paper.authors[0].to_string()
        } else {
            format!("{} et al.", paper.authors[0])
        };
        let journal = paper.journal.as_deref().unwrap_or("Unknown Journal");
        let year = paper.year.as_deref().unwrap_or("n.d.");
        let matched = if item.matched_terms.is_empty() {
            "无直接词项匹配，需人工核验".to_string()
        } else {
            item.matched_terms.join("、")
        };
        let mut entry = format!(
            "[{}；匹配度 {} / 100；命中：{matched}]\n  [PMID: {}](https://pubmed.ncbi.nlm.nih.gov/{}/)\n  {}\n  {} — {}（发表：{}）\n  元数据指纹：{}",
            item.verification.status.label(),
            item.score,
            paper.pmid,
            paper.pmid,
            paper.title,
            authors,
            journal,
            year,
            item.verification.fingerprint,
        );
        if let Some(doi) = paper.doi.as_deref() {
            entry.push_str(&format!("\n  DOI: {doi}"));
        }
        if is_review(paper) {
            entry.push_str(&format!(
                "\n  原始检索截止日：{}",
                extract_search_cutoff(paper.abstract_text.as_deref())
                    .unwrap_or_else(|| "未从 PubMed 摘要提取；引用前须核对原文方法".to_string())
            ));
        }
        lines.push(entry);
    }
    lines.join("\n\n")
}

fn is_review(paper: &Paper) -> bool {
    paper.publication_types.iter().any(|kind| {
        let normalized = normalize(kind);
        normalized.contains("systematic review")
            || normalized.contains("meta analysis")
            || normalized == "review"
    })
}

fn extract_search_cutoff(abstract_text: Option<&str>) -> Option<String> {
    let normalized = normalize(abstract_text?);
    for marker in ["through", "until", "up to", "up until", "from inception to"] {
        let Some(position) = normalized.find(marker) else {
            continue;
        };
        let tail = normalized[position..]
            .split_whitespace()
            .take(12)
            .collect::<Vec<_>>()
            .join(" ");
        if let Some(year) = tail.split_whitespace().find(|token| {
            token.len() == 4
                && token.starts_with("20")
                && token.chars().all(|character| character.is_ascii_digit())
        }) {
            return Some(format!("摘要线索：{marker} {year}"));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paper(pmid: &str, title: &str, abstract_text: &str) -> Paper {
        Paper {
            pmid: pmid.to_string(),
            title: title.to_string(),
            abstract_text: Some(abstract_text.to_string()),
            authors: Vec::new(),
            journal: Some("Test Journal".to_string()),
            year: Some("2025".to_string()),
            volume: None,
            issue: None,
            pages: None,
            doi: None,
            pmcid: None,
            mesh_terms: Vec::new(),
            publication_types: Vec::new(),
        }
    }

    #[test]
    fn topic_matching_ranks_prognosis_studies_above_pathogenesis() {
        let papers = vec![
            paper(
                "11111111",
                "Inflammation pathways in type 2 diabetes",
                "Mechanisms of insulin resistance and beta-cell failure.",
            ),
            paper(
                "22222222",
                "Prognostic risk factors for mortality in type 2 diabetes",
                "A cohort study developed a mortality prediction model.",
            ),
        ];
        let ranked = rank_papers(&papers, "type 2 diabetes prognosis risk factors", 2);
        assert_eq!(ranked[0].paper.pmid, "22222222");
        assert!(ranked[0].score > ranked[1].score);
    }

    #[test]
    fn review_cutoff_is_reported_separately_from_publication_year() {
        let paper = Paper {
            publication_types: vec!["Systematic Review".to_string()],
            ..paper(
                "3",
                "A systematic review",
                "Databases were searched through March 2023.",
            )
        };
        assert!(is_review(&paper));
        assert_eq!(
            extract_search_cutoff(paper.abstract_text.as_deref()),
            Some("摘要线索：through 2023".to_string())
        );
    }

    #[test]
    fn conflicting_claimed_metadata_is_rejected() {
        let paper = Paper {
            doi: Some("10.1000/verified".to_string()),
            ..paper(
                "32946039",
                "Verified stroke rehabilitation trial",
                "Abstract",
            )
        };
        let verification = verify_pubmed_paper(
            &paper,
            Some("A different article about unrelated pharmacology"),
            Some("10.1000/invented"),
        );
        assert_eq!(verification.status, VerificationStatus::Rejected);
        assert!(format_verification(&verification).contains("核验失败 · 禁止引用"));
    }

    #[test]
    fn verified_record_has_a_stable_metadata_fingerprint() {
        let paper = paper(
            "32946039",
            "Verified stroke rehabilitation trial",
            "Abstract",
        );
        let first = verify_pubmed_paper(&paper, None, None);
        let second = verify_pubmed_paper(&paper, None, None);
        assert_eq!(first.status, VerificationStatus::Verified);
        assert_eq!(first.fingerprint, second.fingerprint);
        assert_eq!(first.fingerprint.len(), 16);
    }
}
