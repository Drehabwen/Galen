pub mod citations;
pub mod prompts;
pub mod pubmed;
pub mod types;

use pubmed::PubMedClient;
use types::{CitationStyle, Paper, SearchParams};

pub struct MedicalCore {
    pub pubmed: PubMedClient,
}

impl MedicalCore {
    pub fn new(pubmed_api_key: Option<String>) -> Self {
        Self {
            pubmed: PubMedClient::new(pubmed_api_key),
        }
    }

    pub async fn search_pubmed(
        &self,
        query: &str,
        limit: u32,
    ) -> Result<Vec<Paper>, pubmed::PubMedError> {
        let params = SearchParams {
            query: query.to_string(),
            max_results: limit,
            ..Default::default()
        };
        let result = self.pubmed.search(&params).await?;
        self.pubmed.fetch_articles(&result.pmids).await
    }

    pub async fn fetch_article(&self, pmid: &str) -> Result<Option<Paper>, pubmed::PubMedError> {
        let papers = self
            .pubmed
            .fetch_articles(&[pmid.to_string()])
            .await?;
        Ok(papers.into_iter().next())
    }

    pub async fn fetch_fulltext(
        &self,
        pmids: &[String],
    ) -> Result<Vec<types::FullText>, pubmed::PubMedError> {
        self.pubmed.fetch_fulltext(pmids).await
    }

    pub fn format_citations(&self, papers: &[Paper], style: CitationStyle) -> String {
        citations::format_citations(papers, style)
    }

    pub fn format_single_citation(&self, paper: &Paper, style: CitationStyle) -> String {
        citations::format_single(paper, style)
    }
}
