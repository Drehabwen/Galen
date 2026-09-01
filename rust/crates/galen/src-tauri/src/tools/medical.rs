use api::ToolDefinition;
use async_trait::async_trait;
use medical_core::rehab::{build_rehab_query, RehabFocus, RehabStudyType};
use medical_core::types::CitationStyle;
use medical_core::types::Paper;
use serde_json::{json, Value};

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
    let limit = input["max_results"].as_u64().unwrap_or(10) as u32;
    let papers = match ctx.medical.search_pubmed(query, limit).await {
        Ok(papers) => papers,
        Err(error) => {
            return ToolExecution::from_result(Err(format!("PubMed search error: {error}")))
        }
    };
    ctx.send_event(ChatEvent::SearchResults(papers.clone()));
    let text = if papers.is_empty() {
        "No results found.".into()
    } else {
        let summary: Vec<String> = papers
            .iter()
            .map(|p| {
                let authors = if p.authors.is_empty() {
                    "Unknown".to_string()
                } else if p.authors.len() == 1 {
                    p.authors[0].to_string()
                } else {
                    format!("{} et al.", p.authors[0])
                };
                let journal = p.journal.as_deref().unwrap_or("Unknown Journal");
                let year = p.year.as_deref().unwrap_or("n.d.");
                let doi_str = p
                    .doi
                    .as_deref()
                    .map(|d| format!("\n  DOI: {d}"))
                    .unwrap_or_default();
                format!(
                    "PMID:{}\n  {}\n  {} — {} ({}){}\n",
                    p.pmid, p.title, authors, journal, year, doi_str
                )
            })
            .collect();
        format!("Found {} results:\n\n{}", papers.len(), summary.join("\n"))
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
    let limit = input["max_results"].as_u64().unwrap_or(10) as u32;
    let query = build_rehab_query(topic, focus, study_type);
    let papers = match ctx.medical.search_pubmed(&query, limit).await {
        Ok(papers) => papers,
        Err(error) => {
            return ToolExecution::from_result(Err(format!("PubMed search error: {error}")))
        }
    };
    ctx.send_event(ChatEvent::SearchResults(papers.clone()));
    let text = if papers.is_empty() {
        format!("No results found.\nQuery used: {query}")
    } else {
        let summary = format_paper_entries(&papers);
        format!(
            "Rehabilitation query: {query}\n\nFound {} results:\n\n{}",
            papers.len(),
            summary.join("\n")
        )
    };
    ToolExecution {
        result: Ok(text),
        raw_output: serde_json::to_value(&papers).ok(),
        result_count: Some(papers.len()),
        query: Some(query),
    }
}

// ---------------------------------------------------------------------------
// Shared formatting
// ---------------------------------------------------------------------------

fn format_paper_entries(papers: &[Paper]) -> Vec<String> {
    papers
        .iter()
        .map(|p| {
            let authors = if p.authors.is_empty() {
                "Unknown".to_string()
            } else if p.authors.len() == 1 {
                p.authors[0].to_string()
            } else {
                format!("{} et al.", p.authors[0])
            };
            let journal = p.journal.as_deref().unwrap_or("Unknown Journal");
            let year = p.year.as_deref().unwrap_or("n.d.");
            let doi_str = p
                .doi
                .as_deref()
                .map(|d| format!("\n  DOI: {d}"))
                .unwrap_or_default();
            format!(
                "PMID:{}\n  {}\n  {} — {} ({}){}\n",
                p.pmid, p.title, authors, journal, year, doi_str
            )
        })
        .collect()
}
