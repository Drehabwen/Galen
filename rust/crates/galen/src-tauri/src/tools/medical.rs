use api::ToolDefinition;
use async_trait::async_trait;
use medical_core::types::CitationStyle;
use serde_json::{json, Value};

use super::{GalenTool, ToolContext};
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
        let query = input["query"].as_str().ok_or("Missing 'query' parameter")?;
        let limit = input["max_results"].as_u64().unwrap_or(10) as u32;
        let papers = ctx.medical.search_pubmed(query, limit).await
            .map_err(|e| format!("PubMed search error: {e}"))?;

        ctx.send_event(ChatEvent::SearchResults(papers.clone()));

        if papers.is_empty() {
            Ok("No results found.".into())
        } else {
            let summary: Vec<String> = papers.iter().map(|p| {
                let authors = if p.authors.is_empty() { "Unknown".to_string() }
                    else if p.authors.len() == 1 { p.authors[0].to_string() }
                    else { format!("{} et al.", p.authors[0]) };
                let journal = p.journal.as_deref().unwrap_or("Unknown Journal");
                let year = p.year.as_deref().unwrap_or("n.d.");
                let doi_str = p.doi.as_deref().map(|d| format!("\n  DOI: {d}")).unwrap_or_default();
                format!("PMID:{}\n  {}\n  {} — {} ({}){}\n", p.pmid, p.title, authors, journal, year, doi_str)
            }).collect();
            Ok(format!("Found {} results:\n\n{}", papers.len(), summary.join("\n")))
        }
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
        let paper = ctx.medical.fetch_article(pmid).await
            .map_err(|e| format!("PubMed fetch error: {e}"))?;
        match paper {
            None => Ok(format!("No article found for PMID: {pmid}")),
            Some(p) => Ok(format!(
                "Title: {}\nAuthors: {}\nJournal: {}\nYear: {}\nDOI: {}\n\nAbstract:\n{}",
                p.title,
                p.authors.iter().map(|a| a.to_string()).collect::<Vec<_>>().join(", "),
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
            description: Some("Format papers into a citation style (apa, vancouver, bibtex, ris, mla).".into()),
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
        let pmids: Vec<String> = input["pmids"].as_array()
            .ok_or("Missing 'pmids'")?
            .iter().filter_map(|v| v.as_str().map(String::from)).collect();
        let style = CitationStyle::from_str(
            input["style"].as_str().ok_or("Missing 'style'")?
        ).ok_or("Unknown citation style")?;
        let papers = ctx.medical.pubmed.fetch_articles(&pmids).await
            .map_err(|e| format!("PubMed fetch error: {e}"))?;
        Ok(ctx.medical.format_citations(&papers, style))
    }
}
