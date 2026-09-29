use super::{GalenTool, ToolContext};
use crate::backend::ChatEvent;
use crate::research_task::ResearchNode;
use api::ToolDefinition;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub(crate) const GALEN_SCREENING_FIELD: &str = "_galen_screening";

#[derive(Debug, Clone, Copy)]
enum QuerySource {
    StringField(&'static str),
    JsonField(&'static str),
}

/// One explicitly supported literature-search operation. Recognition is based
/// on host-resolved identity; descriptions and response text are never used.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RecognizedSearch {
    pub provider_id: &'static str,
    query_source: QuerySource,
    result_array_paths: &'static [&'static str],
}

impl RecognizedSearch {
    pub(crate) fn query_from(&self, arguments: &Value) -> String {
        match self.query_source {
            QuerySource::StringField(field) => arguments
                .get(field)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            QuerySource::JsonField(field) => arguments
                .get(field)
                .and_then(|value| serde_json::to_string(value).ok())
                .unwrap_or_default(),
        }
    }

    pub(crate) fn result_count_from(&self, raw: &Value) -> Option<usize> {
        count_at_declared_paths(raw, self.result_array_paths).or_else(|| {
            raw.get("content")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|content| content.get("text").and_then(Value::as_str))
                .filter_map(|text| serde_json::from_str::<Value>(text).ok())
                .find_map(|value| count_at_declared_paths(&value, self.result_array_paths))
        })
    }

    fn result_values_from(&self, raw: &Value) -> Option<Vec<Value>> {
        response_payloads(raw).into_iter().find_map(|payload| {
            self.result_array_paths.iter().find_map(|path| {
                let candidate = if path.is_empty() {
                    &payload
                } else {
                    payload.pointer(path)?
                };
                candidate.as_array().cloned()
            })
        })
    }
}

const PUBMED_PATHS: &[&str] = &[""];
const CROSSREF_PATHS: &[&str] = &[
    "/structuredContent/works",
    "/structuredContent/items",
    "/structuredContent/results",
    "/structuredContent/works",
    "/items",
    "/results",
    "/works",
];
const SEMANTIC_SCHOLAR_PATHS: &[&str] = &[
    "/structuredContent/data",
    "/structuredContent/papers",
    "/structuredContent/results",
    "/data",
    "/papers",
    "/results",
];
const CNKI_PATHS: &[&str] = &[
    "/structuredContent/results",
    "/structuredContent/papers",
    "/structuredContent/items",
    "/results",
    "/papers",
    "/items",
];

#[derive(Debug, Clone, Default)]
pub(crate) struct LiteratureScreening {
    research_question: String,
    required_concepts: Vec<ConceptGroup>,
    excluded_concepts: Vec<ConceptGroup>,
    publication_types: Vec<String>,
    max_results: usize,
}

#[derive(Debug, Clone)]
struct ConceptGroup {
    label: String,
    terms: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum CandidateEligibility {
    Matched,
    Ambiguous,
    Rejected,
}

impl CandidateEligibility {
    fn label(self) -> &'static str {
        match self {
            Self::Matched => "约束匹配",
            Self::Ambiguous => "信息不足 · 禁止直接支撑结论",
            Self::Rejected => "约束不匹配 · 已排除",
        }
    }
}

#[derive(Debug, Clone)]
struct LiteratureCandidate {
    provider_rank: usize,
    title: String,
    abstract_text: Option<String>,
    authors: Vec<String>,
    year: Option<String>,
    venue: Option<String>,
    doi: Option<String>,
    url: Option<String>,
    publication_types: Vec<String>,
    lexical_score: u8,
    eligibility: CandidateEligibility,
    screening_reasons: Vec<String>,
}

#[derive(Debug)]
pub(crate) struct NormalizedMcpSearch {
    pub report: String,
    pub result_count: usize,
}

pub(crate) fn augment_mcp_search_definition(
    server_name: &str,
    tool_name: &str,
    description: Option<String>,
    mut schema: Value,
) -> (Option<String>, Value) {
    if recognized_mcp_search(server_name, tool_name).is_none() {
        return (description, schema);
    }
    if let Some(root) = schema.as_object_mut() {
        root.entry("type").or_insert_with(|| json!("object"));
        let properties = root
            .entry("properties")
            .or_insert_with(|| json!({}))
            .as_object_mut();
        if let Some(properties) = properties {
        properties.insert(
            GALEN_SCREENING_FIELD.to_string(),
            json!({
                "type": "object",
                "description": "Galen host-only post-retrieval screening. This object is removed before the provider call. Use it for scientific eligibility constraints instead of over-constraining the provider query.",
                "properties": {
                    "research_question": {"type": "string"},
                    "required_concepts": {"type": "array", "items": concept_group_schema()},
                    "excluded_concepts": {"type": "array", "items": concept_group_schema()},
                    "publication_types": {"type": "array", "items": {"type": "string"}},
                    "max_results": {"type": "integer", "minimum": 1, "maximum": 20}
                }
            }),
        );
        }
    }
    let suffix = " Galen requirement: keep the provider query broad and put population/intervention/outcome exclusions in _galen_screening; results that cannot be normalized or screened are not promoted as evidence.";
    let description = Some(format!("{}{}", description.unwrap_or_default(), suffix));
    (description, schema)
}

fn concept_group_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "label": {"type": "string"},
            "terms": {"type": "array", "items": {"type": "string"}}
        },
        "required": ["label", "terms"]
    })
}

pub(crate) fn split_mcp_search_arguments(
    input: &Value,
) -> Result<(Value, LiteratureScreening), String> {
    let mut provider_input = input.clone();
    let screening_value = provider_input
        .as_object_mut()
        .and_then(|object| object.remove(GALEN_SCREENING_FIELD));
    let screening = LiteratureScreening::from_value(screening_value.as_ref())?;
    Ok((provider_input, screening))
}

impl LiteratureScreening {
    fn from_value(value: Option<&Value>) -> Result<Self, String> {
        let Some(value) = value else {
            return Ok(Self {
                max_results: 8,
                ..Self::default()
            });
        };
        let object = value
            .as_object()
            .ok_or("'_galen_screening' must be an object")?;
        Ok(Self {
            research_question: object
                .get("research_question")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim()
                .to_string(),
            required_concepts: parse_concept_groups(object.get("required_concepts"))?,
            excluded_concepts: parse_concept_groups(object.get("excluded_concepts"))?,
            publication_types: parse_string_values(object.get("publication_types"))?,
            max_results: object
                .get("max_results")
                .and_then(Value::as_u64)
                .unwrap_or(8)
                .clamp(1, 20) as usize,
        })
    }

    fn has_hard_constraints(&self) -> bool {
        !self.required_concepts.is_empty()
            || !self.excluded_concepts.is_empty()
            || !self.publication_types.is_empty()
    }
}

fn parse_concept_groups(value: Option<&Value>) -> Result<Vec<ConceptGroup>, String> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let groups = value
        .as_array()
        .ok_or("screening concept groups must be arrays")?;
    if groups.len() > 12 {
        return Err("screening accepts at most 12 concept groups".into());
    }
    groups
        .iter()
        .map(|group| {
            let label = group
                .get("label")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|label| !label.is_empty())
                .ok_or("every screening concept needs a label")?;
            let terms = parse_string_values(group.get("terms"))?;
            if terms.is_empty() {
                return Err(format!("screening concept '{label}' needs at least one term"));
            }
            Ok(ConceptGroup {
                label: label.to_string(),
                terms,
            })
        })
        .collect()
}

fn parse_string_values(value: Option<&Value>) -> Result<Vec<String>, String> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let values = value.as_array().ok_or("screening values must be arrays")?;
    Ok(values
        .iter()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .take(12)
        .map(ToString::to_string)
        .collect())
}

pub(crate) fn normalize_mcp_search(
    search: RecognizedSearch,
    raw: &Value,
    query: &str,
    screening: &LiteratureScreening,
) -> Result<NormalizedMcpSearch, String> {
    let values = search.result_values_from(raw).ok_or_else(|| {
        format!(
            "invalid response: {} search payload did not contain a declared result array",
            search.provider_id
        )
    })?;
    if values.is_empty() {
        return Ok(NormalizedMcpSearch {
            report: format!(
                "来源：{}\n检索式：{}\n规范化结果：0\n本次是成功的零结果检索，不代表未配置、不可用或检索失败。",
                search.provider_id, query
            ),
            result_count: 0,
        });
    }

    let mut candidates = values
        .iter()
        .enumerate()
        .filter_map(|(rank, value)| normalize_candidate(search.provider_id, rank, value))
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return Err(format!(
            "invalid response: {} returned {} records but none had a recognizable title",
            search.provider_id,
            values.len()
        ));
    }
    let ranking_anchor = if screening.research_question.is_empty() {
        query
    } else {
        &screening.research_question
    };
    for candidate in &mut candidates {
        candidate.lexical_score = lexical_score(candidate, ranking_anchor);
        let (eligibility, reasons) = screen_candidate(candidate, screening);
        candidate.eligibility = eligibility;
        candidate.screening_reasons = reasons;
    }
    candidates.sort_by(|left, right| {
        left.eligibility
            .cmp(&right.eligibility)
            .then_with(|| right.lexical_score.cmp(&left.lexical_score))
            .then_with(|| left.provider_rank.cmp(&right.provider_rank))
    });
    Ok(render_normalized_search(
        search.provider_id,
        query,
        candidates,
        screening.max_results,
    ))
}

fn response_payloads(raw: &Value) -> Vec<Value> {
    let mut payloads = vec![raw.clone()];
    if let Some(structured) = raw.get("structuredContent") {
        payloads.push(structured.clone());
    }
    if let Some(content) = raw.get("content").and_then(Value::as_array) {
        for text in content
            .iter()
            .filter_map(|item| item.get("text").and_then(Value::as_str))
        {
            if let Ok(value) = serde_json::from_str::<Value>(text) {
                payloads.push(value);
            }
        }
    }
    payloads
}

const BUILTIN_SEARCHES: &[(&str, RecognizedSearch)] = &[
    (
        "search_pubmed",
        RecognizedSearch {
            provider_id: "pubmed",
            query_source: QuerySource::StringField("query"),
            result_array_paths: PUBMED_PATHS,
        },
    ),
    (
        "search_rehab_literature",
        RecognizedSearch {
            provider_id: "pubmed",
            query_source: QuerySource::StringField("topic"),
            result_array_paths: PUBMED_PATHS,
        },
    ),
];

const MCP_SEARCHES: &[(&str, &str, RecognizedSearch)] = &[
    (
        "crossref",
        "crossref_search_works",
        RecognizedSearch {
            provider_id: "crossref",
            query_source: QuerySource::StringField("query"),
            result_array_paths: CROSSREF_PATHS,
        },
    ),
    (
        "crossref",
        "search_papers",
        RecognizedSearch {
            provider_id: "crossref",
            query_source: QuerySource::StringField("query"),
            result_array_paths: CROSSREF_PATHS,
        },
    ),
    (
        "semantic-scholar",
        "semantic_scholar_search_papers",
        RecognizedSearch {
            provider_id: "semantic-scholar",
            query_source: QuerySource::StringField("query"),
            result_array_paths: SEMANTIC_SCHOLAR_PATHS,
        },
    ),
    (
        "semantic-scholar",
        "semantic_scholar_bulk_search",
        RecognizedSearch {
            provider_id: "semantic-scholar",
            query_source: QuerySource::StringField("query"),
            result_array_paths: SEMANTIC_SCHOLAR_PATHS,
        },
    ),
    (
        "semantic-scholar",
        "search_papers",
        RecognizedSearch {
            provider_id: "semantic-scholar",
            query_source: QuerySource::StringField("query"),
            result_array_paths: SEMANTIC_SCHOLAR_PATHS,
        },
    ),
    (
        "cnki",
        "cnki_search",
        RecognizedSearch {
            provider_id: "cnki",
            query_source: QuerySource::StringField("query"),
            result_array_paths: CNKI_PATHS,
        },
    ),
    (
        "cnki",
        "cnki_structured_search",
        RecognizedSearch {
            provider_id: "cnki",
            query_source: QuerySource::JsonField("conditions"),
            result_array_paths: CNKI_PATHS,
        },
    ),
];

pub(crate) fn recognized_builtin_search(tool_name: &str) -> Option<RecognizedSearch> {
    BUILTIN_SEARCHES
        .iter()
        .find_map(|(name, search)| (*name == tool_name).then_some(*search))
}

pub(crate) fn recognized_mcp_search(
    server_name: &str,
    tool_name: &str,
) -> Option<RecognizedSearch> {
    MCP_SEARCHES.iter().find_map(|(server, tool, search)| {
        (*server == server_name && *tool == tool_name).then_some(*search)
    })
}

fn count_at_declared_paths(value: &Value, paths: &[&str]) -> Option<usize> {
    paths.iter().find_map(|path| {
        let candidate = if path.is_empty() {
            value
        } else {
            value.pointer(path)?
        };
        candidate.as_array().map(Vec::len)
    })
}

pub(crate) fn is_recognized_qualified_mcp_search(name: &str) -> bool {
    crate::mcp_client::parse_qualified_tool_name(name)
        .and_then(|(server, tool)| recognized_mcp_search(server, tool))
        .is_some()
}

fn normalize_candidate(
    provider_id: &str,
    provider_rank: usize,
    value: &Value,
) -> Option<LiteratureCandidate> {
    let mut sources = vec![value];
    if let Some(metadata) = value.get("metadata") {
        sources.push(metadata);
    }
    let title = first_text(
        &sources,
        &["title", "paperTitle", "paper_title", "题名", "标题"],
    )?;
    let abstract_text = first_text(
        &sources,
        &["abstract", "abstractText", "summary", "摘要"],
    );
    let authors = first_authors(&sources);
    let year = first_year(&sources);
    let venue = first_text(
        &sources,
        &[
            "containerTitle",
            "venue",
            "journal",
            "source",
            "期刊",
            "来源",
        ],
    );
    let doi = first_text(&sources, &["doi", "DOI"]).or_else(|| {
        sources.iter().find_map(|source| {
            source
                .get("externalIds")
                .or_else(|| source.get("external_ids"))
                .and_then(|ids| ids.get("DOI").or_else(|| ids.get("doi")))
                .and_then(value_text)
        })
    });
    let url = first_text(&sources, &["url", "URL", "link", "链接"]).or_else(|| {
        first_text(&sources, &["paperId", "paper_id"]).map(|paper_id| {
            format!("https://www.semanticscholar.org/paper/{paper_id}")
        })
    });
    let publication_types = first_string_list(
        &sources,
        &[
            "publicationTypes",
            "publication_types",
            "type",
            "documentType",
            "文献类型",
        ],
    );

    Some(LiteratureCandidate {
        provider_rank,
        title,
        abstract_text,
        authors,
        year,
        venue,
        doi,
        url,
        publication_types,
        lexical_score: 0,
        eligibility: CandidateEligibility::Ambiguous,
        screening_reasons: vec![format!("已规范化为 {provider_id} 候选记录")],
    })
}

fn first_text(sources: &[&Value], keys: &[&str]) -> Option<String> {
    sources.iter().find_map(|source| {
        keys.iter()
            .find_map(|key| source.get(*key).and_then(value_text))
    })
}

fn value_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => non_empty(text),
        Value::Number(number) => Some(number.to_string()),
        Value::Array(values) => values.iter().find_map(value_text),
        _ => None,
    }
}

fn non_empty(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn first_authors(sources: &[&Value]) -> Vec<String> {
    for source in sources {
        let Some(value) = ["authors", "author", "作者"]
            .iter()
            .find_map(|key| source.get(*key))
        else {
            continue;
        };
        let values = value.as_array().cloned().unwrap_or_else(|| vec![value.clone()]);
        let authors = values
            .iter()
            .filter_map(|author| match author {
                Value::String(name) => non_empty(name),
                Value::Object(_) => {
                    if let Some(name) = author.get("name").and_then(value_text) {
                        return Some(name);
                    }
                    let given = author
                        .get("given")
                        .or_else(|| author.get("firstName"))
                        .and_then(value_text)
                        .unwrap_or_default();
                    let family = author
                        .get("family")
                        .or_else(|| author.get("lastName"))
                        .and_then(value_text)
                        .unwrap_or_default();
                    non_empty(&format!("{given} {family}"))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        if !authors.is_empty() {
            return authors;
        }
    }
    Vec::new()
}

fn first_year(sources: &[&Value]) -> Option<String> {
    if let Some(year) = first_text(sources, &["year", "publicationYear", "出版年", "年份"]) {
        return Some(year);
    }
    for source in sources {
        for key in ["published", "publicationDate", "publication_date", "发表时间"] {
            let Some(value) = source.get(key) else {
                continue;
            };
            if let Some(year) = value.get("year").and_then(value_text) {
                return Some(year);
            }
            if let Some(text) = value_text(value) {
                if let Some(year) = extract_year(&text) {
                    return Some(year);
                }
            }
        }
    }
    None
}

fn extract_year(value: &str) -> Option<String> {
    value
        .split(|character: char| !character.is_ascii_digit())
        .find(|part| part.len() == 4 && matches!(part.get(..2), Some("19") | Some("20")))
        .map(ToString::to_string)
}

fn first_string_list(sources: &[&Value], keys: &[&str]) -> Vec<String> {
    for source in sources {
        let Some(value) = keys.iter().find_map(|key| source.get(*key)) else {
            continue;
        };
        let values = match value {
            Value::Array(values) => values.iter().filter_map(value_text).collect::<Vec<_>>(),
            _ => value_text(value).into_iter().collect(),
        };
        if !values.is_empty() {
            return values;
        }
    }
    Vec::new()
}

fn normalize_text(value: &str) -> String {
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

fn phrase_matches(text: &str, phrase: &str) -> bool {
    if phrase
        .chars()
        .any(|character| character.is_alphanumeric() && !character.is_ascii())
    {
        text.contains(phrase)
    } else {
        text == phrase || format!(" {text} ").contains(&format!(" {phrase} "))
    }
}

fn concept_matches(concept: &ConceptGroup, searchable: &str) -> bool {
    concept.terms.iter().any(|term| {
        let term = normalize_text(term);
        !term.is_empty() && phrase_matches(searchable, &term)
    })
}

fn candidate_searchable(candidate: &LiteratureCandidate) -> String {
    normalize_text(&format!(
        "{} {} {} {}",
        candidate.title,
        candidate.abstract_text.as_deref().unwrap_or_default(),
        candidate.venue.as_deref().unwrap_or_default(),
        candidate.publication_types.join(" ")
    ))
}

fn lexical_terms(question: &str) -> Vec<String> {
    const STOP_WORDS: &[&str] = &[
        "and", "or", "not", "the", "with", "for", "from", "after", "before", "study",
        "research", "paper", "evidence",
    ];
    let mut terms = normalize_text(question)
        .split_whitespace()
        .filter(|term| term.chars().count() >= 2 && !STOP_WORDS.contains(term))
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    terms.sort();
    terms.dedup();
    terms
}

fn lexical_score(candidate: &LiteratureCandidate, question: &str) -> u8 {
    let terms = lexical_terms(question);
    if terms.is_empty() {
        return 0;
    }
    let title = normalize_text(&candidate.title);
    let abstract_text = normalize_text(candidate.abstract_text.as_deref().unwrap_or_default());
    let weighted_hits = terms
        .iter()
        .map(|term| {
            let title_hit = phrase_matches(&title, term);
            let abstract_hit = phrase_matches(&abstract_text, term);
            u32::from(title_hit) * 5 + u32::from(abstract_hit) * 2
        })
        .sum::<u32>();
    let matched = terms
        .iter()
        .filter(|term| phrase_matches(&title, term) || phrase_matches(&abstract_text, term))
        .count();
    ((matched as u32 * 70 / terms.len() as u32) + weighted_hits.min(30)).min(100) as u8
}

fn screen_candidate(
    candidate: &LiteratureCandidate,
    screening: &LiteratureScreening,
) -> (CandidateEligibility, Vec<String>) {
    if !screening.has_hard_constraints() {
        return (
            CandidateEligibility::Ambiguous,
            vec!["未提供 Galen 硬约束；外部来源只作为待筛查候选".to_string()],
        );
    }
    let searchable = candidate_searchable(candidate);
    let excluded = screening
        .excluded_concepts
        .iter()
        .filter(|concept| concept_matches(concept, &searchable))
        .map(|concept| concept.label.clone())
        .collect::<Vec<_>>();
    if !excluded.is_empty() {
        return (
            CandidateEligibility::Rejected,
            vec![format!("命中排除概念：{}", excluded.join("、"))],
        );
    }
    if !screening.publication_types.is_empty() {
        if candidate.publication_types.is_empty() {
            return (
                CandidateEligibility::Ambiguous,
                vec!["来源未提供文献类型，不能确认类型约束".to_string()],
            );
        }
        let actual = normalize_text(&candidate.publication_types.join(" "));
        if !screening
            .publication_types
            .iter()
            .any(|expected| phrase_matches(&actual, &normalize_text(expected)))
        {
            return (
                CandidateEligibility::Rejected,
                vec![format!(
                    "文献类型不匹配：{}",
                    candidate.publication_types.join("、")
                )],
            );
        }
    }
    let missing = screening
        .required_concepts
        .iter()
        .filter(|concept| !concept_matches(concept, &searchable))
        .map(|concept| concept.label.clone())
        .collect::<Vec<_>>();
    if missing.is_empty() {
        return (
            CandidateEligibility::Matched,
            vec!["所有必含概念均已命中".to_string()],
        );
    }
    let reason = format!("缺少必含概念：{}", missing.join("、"));
    if candidate.abstract_text.is_none() {
        (CandidateEligibility::Ambiguous, vec![reason])
    } else {
        (CandidateEligibility::Rejected, vec![reason])
    }
}

fn render_normalized_search(
    provider_id: &str,
    query: &str,
    candidates: Vec<LiteratureCandidate>,
    max_results: usize,
) -> NormalizedMcpSearch {
    let result_count = candidates.len();
    let rejected_count = candidates
        .iter()
        .filter(|candidate| candidate.eligibility == CandidateEligibility::Rejected)
        .count();
    let ambiguous_count = candidates
        .iter()
        .filter(|candidate| candidate.eligibility == CandidateEligibility::Ambiguous)
        .count();
    let mut reason_counts = BTreeMap::new();
    for reason in candidates
        .iter()
        .filter(|candidate| candidate.eligibility == CandidateEligibility::Rejected)
        .flat_map(|candidate| candidate.screening_reasons.iter())
    {
        *reason_counts.entry(reason.clone()).or_insert(0_usize) += 1;
    }
    let shown = candidates
        .iter()
        .filter(|candidate| candidate.eligibility != CandidateEligibility::Rejected)
        .take(max_results)
        .collect::<Vec<_>>();
    let mut lines = vec![
        format!("来源：{provider_id}"),
        format!("检索式：{query}"),
        format!(
            "规范化候选：{result_count}；展示：{}；排除：{rejected_count}；信息不足：{ambiguous_count}",
            shown.len()
        ),
        "安全边界：外部来源记录只完成元数据规范化与确定性筛查；“信息不足”记录禁止直接支撑结论，应通过 DOI/来源详情进一步核验。".to_string(),
    ];
    if !reason_counts.is_empty() {
        lines.push(format!(
            "排除原因汇总：{}",
            reason_counts
                .iter()
                .map(|(reason, count)| format!("{reason}（{count}）"))
                .collect::<Vec<_>>()
                .join("；")
        ));
    }
    if shown.is_empty() {
        lines.push("没有候选通过当前筛查；不要把被排除记录作为证据。".to_string());
    }
    for candidate in shown {
        let authors = if candidate.authors.is_empty() {
            "作者未提供".to_string()
        } else {
            candidate.authors.iter().take(4).cloned().collect::<Vec<_>>().join("、")
        };
        let mut entry = format!(
            "[{}；词项覆盖 {} / 100]\n  {}\n  {} — {}（{}）\n  筛查依据：{}\n  摘要线索：{}",
            candidate.eligibility.label(),
            candidate.lexical_score,
            candidate.title,
            authors,
            candidate.venue.as_deref().unwrap_or("来源未提供"),
            candidate.year.as_deref().unwrap_or("年份未提供"),
            candidate.screening_reasons.join("；"),
            excerpt(candidate.abstract_text.as_deref()),
        );
        if let Some(doi) = &candidate.doi {
            entry.push_str(&format!("\n  DOI: {doi}"));
        }
        if let Some(url) = &candidate.url {
            entry.push_str(&format!("\n  URL: {url}"));
        }
        lines.push(entry);
    }
    NormalizedMcpSearch {
        report: lines.join("\n\n"),
        result_count,
    }
}

fn excerpt(value: Option<&str>) -> String {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return "来源未提供摘要".to_string();
    };
    let mut text = value.chars().take(240).collect::<String>();
    if value.chars().count() > 240 {
        text.push('…');
    }
    text.replace('\n', " ")
}

pub struct CreateResearchPlan;

pub struct UpdateResearchContext;

#[derive(Debug, Deserialize)]
struct PlanInput {
    title: String,
    goal: String,
    nodes: Vec<PlanNodeInput>,
}

#[derive(Debug, Deserialize)]
struct PlanNodeInput {
    id: String,
    index: String,
    title: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    node_type: Option<String>,
    #[serde(default)]
    depends_on: Vec<String>,
}

#[async_trait]
impl GalenTool for CreateResearchPlan {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "create_research_plan".into(),
            description: Some(
                "Create the durable structured research task and canvas nodes before delivering files. Use when the user asks for a multi-node research plan."
                    .into(),
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "title": {"type": "string"},
                    "goal": {"type": "string"},
                    "nodes": {
                        "type": "array",
                        "minItems": 1,
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": {"type": "string"},
                                "index": {"type": "string"},
                                "title": {"type": "string"},
                                "description": {"type": "string"},
                                "node_type": {"type": "string"},
                                "depends_on": {"type": "array", "items": {"type": "string"}}
                            },
                            "required": ["id", "index", "title"]
                        }
                    }
                },
                "required": ["title", "goal", "nodes"]
            }),
        }
    }

    fn is_write(&self) -> bool {
        true
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String, String> {
        let input: PlanInput =
            serde_json::from_value(input).map_err(|error| format!("研究计划参数无效: {error}"))?;
        if input.nodes.is_empty() {
            return Err("研究计划至少需要一个节点".to_string());
        }
        let root = ctx
            .workspace_root
            .lock()
            .map_err(|error| format!("工作区锁失败: {error}"))?
            .clone()
            .ok_or("请先选择工作区")?;
        let nodes = input
            .nodes
            .into_iter()
            .map(|node| ResearchNode {
                id: node.id,
                index: node.index,
                title: node.title,
                description: node.description,
                node_type: node.node_type.unwrap_or_else(|| "planning".to_string()),
                status: "pending".to_string(),
                owner: Some("Galen".to_string()),
                inputs: Vec::new(),
                outputs: Vec::new(),
                depends_on: node.depends_on,
                tags: Vec::new(),
                risk_level: Some("low".to_string()),
                approval_required: false,
                sub_sessions: Vec::new(),
                result: None,
                evidence: Vec::new(),
                extra: BTreeMap::new(),
            })
            .collect();
        let task = crate::research_task::create_task(&root, input.title, input.goal, nodes)?;
        ctx.send_event(ChatEvent::ResearchTaskUpdated(task.clone()));
        serde_json::to_string(&task).map_err(|error| format!("序列化研究任务失败: {error}"))
    }
}

#[async_trait]
impl GalenTool for UpdateResearchContext {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "update_research_context".into(),
            description: Some(
                "Merge new research scope, search constraints, variables, or timepoints into the active project without dropping earlier decisions. Use excluded_scope for an intentional removal.".into(),
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "researchQuestion": {"type": "string"},
                    "scope": {"type": "array", "items": {"type": "string"}},
                    "constraints": {"type": "array", "items": {"type": "string"}},
                    "variables": {"type": "array", "items": {"type": "string"}},
                    "timepoints": {"type": "array", "items": {"type": "string"}},
                    "activeArtifacts": {"type": "array", "items": {"type": "string"}},
                    "excludedScope": {"type": "array", "items": {"type": "string"}},
                    "revisionNote": {"type": "string"}
                }
            }),
        }
    }

    fn is_write(&self) -> bool {
        true
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String, String> {
        let patch: crate::research_task::ActiveResearchContext =
            serde_json::from_value(input).map_err(|error| format!("context patch invalid: {error}"))?;
        let root = ctx
            .workspace_root
            .lock()
            .map_err(|error| format!("workspace lock failed: {error}"))?
            .clone()
            .ok_or("workspace is not selected")?;
        let current = crate::research_task::load_active_task(&root)?
            .ok_or("no active research task")?;
        let task = crate::research_task::update_active_context(
            &root,
            &current.task_id,
            current.revision,
            patch,
        )?;
        crate::pi_event::append_event(
            &root,
            &task.task_id,
            None,
            crate::pi_event::PiEventKind::ContextUpdated,
            &format!("context-updated:{}", task.revision),
            json!({"revision": task.revision, "activeContext": task.active_context}),
        )?;
        ctx.send_event(ChatEvent::ResearchTaskUpdated(task.clone()));
        serde_json::to_string(&task).map_err(|error| format!("context update serialization failed: {error}"))
    }
}

#[cfg(test)]
mod search_catalog_tests {
    use super::*;

    fn screening(required_label: &str, required_terms: &[&str]) -> LiteratureScreening {
        LiteratureScreening {
            research_question: required_terms.join(" "),
            required_concepts: vec![ConceptGroup {
                label: required_label.to_string(),
                terms: required_terms.iter().map(|term| term.to_string()).collect(),
            }],
            max_results: 8,
            ..LiteratureScreening::default()
        }
    }

    #[test]
    fn literature_search_catalog_is_explicit_and_excludes_cnki_non_search_tools() {
        // Broad prefix/name guessing would incorrectly record login, download, or reading as search.
        assert_eq!(
            recognized_builtin_search("search_pubmed")
                .unwrap()
                .provider_id,
            "pubmed"
        );
        assert_eq!(
            recognized_mcp_search("crossref", "crossref_search_works")
                .unwrap()
                .provider_id,
            "crossref"
        );
        assert_eq!(
            recognized_mcp_search("semantic-scholar", "semantic_scholar_search_papers")
                .unwrap()
                .provider_id,
            "semantic-scholar"
        );
        assert_eq!(
            recognized_mcp_search("cnki", "cnki_structured_search")
                .unwrap()
                .provider_id,
            "cnki"
        );
        for tool in ["cnki_login", "cnki_download_paper", "cnki_read_online_html"] {
            assert!(recognized_mcp_search("cnki", tool).is_none(), "{tool}");
        }
        assert!(recognized_mcp_search("unrelated-server", "crossref_search_works").is_none());
    }

    #[test]
    fn context_update_tool_is_a_write_operation() {
        let tool = UpdateResearchContext;
        assert!(tool.is_write());
        assert!(tool.definition().name == "update_research_context");
    }

    #[test]
    fn crossref_works_are_normalized_and_population_mismatch_is_withheld() {
        let search = recognized_mcp_search("crossref", "crossref_search_works").unwrap();
        let raw = json!({
            "structuredContent": {
                "works": [
                    {
                        "doi": "10.1000/sci",
                        "title": "Robot-assisted gait training after spinal cord injury",
                        "authors": [{"given": "Ada", "family": "Lee"}],
                        "published": {"year": 2025},
                        "containerTitle": "Rehabilitation Journal",
                        "abstract": "Adults with spinal cord injury completed exoskeleton gait training.",
                        "type": "journal-article"
                    },
                    {
                        "doi": "10.1000/stroke",
                        "title": "Robot-assisted gait training after stroke",
                        "abstract": "Adults with stroke completed robotic gait training.",
                        "type": "journal-article"
                    }
                ]
            }
        });
        let normalized = normalize_mcp_search(
            search,
            &raw,
            "robot assisted gait training",
            &screening("population: spinal cord injury", &["spinal cord injury", "SCI"]),
        )
        .unwrap();

        assert_eq!(normalized.result_count, 2);
        assert!(normalized.report.contains("10.1000/sci"));
        assert!(!normalized.report.contains("10.1000/stroke"));
        assert!(normalized.report.contains("排除：1"));
    }

    #[test]
    fn semantic_scholar_and_cnki_aliases_normalize_to_the_same_report_contract() {
        let semantic = recognized_mcp_search(
            "semantic-scholar",
            "semantic_scholar_search_papers",
        )
        .unwrap();
        let semantic_raw = json!({
            "content": [{"type": "text", "text": serde_json::to_string(&json!({
                "data": [{
                    "paperId": "s2-paper",
                    "title": "Heart rate variability during recovery",
                    "abstract": "Athletes were monitored after exercise.",
                    "authors": [{"name": "Lin Chen"}],
                    "year": 2024,
                    "externalIds": {"DOI": "10.1000/hrv"},
                    "publicationTypes": ["JournalArticle"]
                }]
            })).unwrap()}]
        });
        let semantic_report = normalize_mcp_search(
            semantic,
            &semantic_raw,
            "heart rate variability recovery",
            &screening("outcome: HRV", &["heart rate variability", "HRV"]),
        )
        .unwrap();
        assert!(semantic_report.report.contains("10.1000/hrv"));
        assert!(semantic_report.report.contains("约束匹配"));

        let cnki = recognized_mcp_search("cnki", "cnki_search").unwrap();
        let cnki_raw = json!({
            "structuredContent": {
                "results": [{
                    "题名": "运动后心率变异性恢复研究",
                    "摘要": "观察运动员训练后心率变异性恢复。",
                    "作者": ["陈林"],
                    "期刊": "中国运动医学杂志",
                    "年份": "2023",
                    "链接": "https://example.invalid/cnki/1",
                    "文献类型": "期刊论文"
                }]
            }
        });
        let cnki_report = normalize_mcp_search(
            cnki,
            &cnki_raw,
            "心率变异性 恢复",
            &screening("结局：心率变异性", &["心率变异性", "HRV"]),
        )
        .unwrap();
        assert!(cnki_report.report.contains("运动后心率变异性恢复研究"));
        assert!(cnki_report.report.contains("约束匹配"));
    }

    #[test]
    fn host_screening_is_exposed_to_the_model_but_removed_before_provider_call() {
        let (description, schema) = augment_mcp_search_definition(
            "crossref",
            "crossref_search_works",
            Some("Search Crossref".into()),
            json!({"type": "object", "properties": {"query": {"type": "string"}}}),
        );
        assert!(description.unwrap().contains(GALEN_SCREENING_FIELD));
        assert!(schema["properties"].get(GALEN_SCREENING_FIELD).is_some());

        let input = json!({
            "query": "stroke rehabilitation",
            (GALEN_SCREENING_FIELD): {
                "required_concepts": [{"label": "population", "terms": ["stroke"]}]
            }
        });
        let (provider_input, screening) = split_mcp_search_arguments(&input).unwrap();
        assert_eq!(provider_input, json!({"query": "stroke rehabilitation"}));
        assert!(screening.has_hard_constraints());
    }

    #[test]
    fn unknown_success_payload_is_not_treated_as_screened_evidence() {
        let search = recognized_mcp_search("crossref", "crossref_search_works").unwrap();
        let error = normalize_mcp_search(
            search,
            &json!({"structuredContent": {"unexpected": []}}),
            "stroke",
            &LiteratureScreening::default(),
        )
        .unwrap_err();
        assert!(error.contains("invalid response"));
    }
}
