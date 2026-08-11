use crate::types::{
    Author, FullText, FullTextSection, MeSHTerm, Paper, SearchParams, SearchResult,
};
use reqwest::Client;
use serde::Deserialize;

const ESEARCH_URL: &str = "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/esearch.fcgi";
const EFETCH_URL: &str = "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/efetch.fcgi";
const ESPELL_URL: &str = "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/espell.fcgi";
const TOOL_NAME: &str = "claw-medical";
const EMAIL: &str = "medical@claw.dev";

#[derive(Debug, Clone)]
pub struct PubMedClient {
    client: Client,
    api_key: Option<String>,
}

impl PubMedClient {
    pub fn new(api_key: Option<String>) -> Self {
        Self {
            client: Client::new(),
            api_key,
        }
    }

    pub async fn search(&self, params: &SearchParams) -> Result<SearchResult, PubMedError> {
        let retmax = params.max_results.to_string();
        let sort = params.sort.as_deref().unwrap_or("relevance").to_string();
        let mut query_params: Vec<(&str, &str)> = vec![
            ("db", "pubmed"),
            ("term", params.query.as_str()),
            ("retmax", &retmax),
            ("retmode", "json"),
            ("sort", &sort),
            ("tool", TOOL_NAME),
            ("email", EMAIL),
        ];

        if let Some(ref key) = self.api_key {
            query_params.push(("api_key", key.as_str()));
        }
        if let Some(ref min) = params.min_date {
            query_params.push(("mindate", min.as_str()));
            query_params.push(("datetype", "pdat"));
        }
        if let Some(ref max) = params.max_date {
            query_params.push(("maxdate", max.as_str()));
            query_params.push(("datetype", "pdat"));
        }

        let url = build_url(ESEARCH_URL, &query_params);
        let response = self.client.get(&url).send().await?;
        let body: ESearchResult = response.json().await?;

        Ok(SearchResult {
            total_count: body.esearchresult.count.parse().unwrap_or(0),
            pmids: body.esearchresult.idlist,
        })
    }

    pub async fn fetch_articles(
        &self,
        pmids: &[String],
    ) -> Result<Vec<Paper>, PubMedError> {
        if pmids.is_empty() {
            return Ok(Vec::new());
        }

        let id_list = pmids.join(",");
        let mut query_params: Vec<(&str, &str)> = vec![
            ("db", "pubmed"),
            ("id", &id_list),
            ("retmode", "xml"),
            ("rettype", "abstract"),
            ("tool", TOOL_NAME),
            ("email", EMAIL),
        ];

        if let Some(ref key) = self.api_key {
            query_params.push(("api_key", key.as_str()));
        }

        let url = build_url(EFETCH_URL, &query_params);
        let response = self.client.get(&url).send().await?;
        let xml = response.text().await?;
        parse_pubmed_xml(&xml)
    }

    pub async fn fetch_fulltext(
        &self,
        pmids: &[String],
    ) -> Result<Vec<FullText>, PubMedError> {
        if pmids.is_empty() {
            return Ok(Vec::new());
        }

        let id_list = pmids.join(",");
        let mut query_params: Vec<(&str, &str)> = vec![
            ("db", "pmc"),
            ("id", &id_list),
            ("retmode", "xml"),
            ("tool", TOOL_NAME),
            ("email", EMAIL),
        ];

        if let Some(ref key) = self.api_key {
            query_params.push(("api_key", key.as_str()));
        }

        let url = build_url(EFETCH_URL, &query_params);
        let response = self.client.get(&url).send().await?;
        let xml = response.text().await?;
        parse_pmc_fulltext(&xml)
    }

    pub async fn spell_check(&self, query: &str) -> Result<String, PubMedError> {
        let query_params = vec![
            ("db", "pubmed"),
            ("term", query),
            ("tool", TOOL_NAME),
            ("email", EMAIL),
        ];
        let url = build_url(ESPELL_URL, &query_params);
        let response = self.client.get(&url).send().await?;
        let body: ESpellResult = response.json().await?;
        Ok(body.esearchresult.correctedquery.unwrap_or_else(|| query.to_string()))
    }
}

fn build_url(base: &str, params: &[(&str, &str)]) -> String {
    let query: Vec<String> = params
        .iter()
        .map(|(k, v)| format!("{}={}", urlencoding(k), urlencoding(v)))
        .collect();
    format!("{}?{}", base, query.join("&"))
}

fn urlencoding(s: &str) -> String {
    s.replace(' ', "+")
        .replace('[', "%5B")
        .replace(']', "%5D")
        .replace('\"', "%22")
}

#[derive(Debug)]
pub enum PubMedError {
    Http(reqwest::Error),
    Parse(String),
    NoResults,
}

impl std::fmt::Display for PubMedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Http(e) => write!(f, "PubMed HTTP error: {e}"),
            Self::Parse(e) => write!(f, "PubMed parse error: {e}"),
            Self::NoResults => write!(f, "no results found"),
        }
    }
}

impl std::error::Error for PubMedError {}

impl From<reqwest::Error> for PubMedError {
    fn from(e: reqwest::Error) -> Self {
        Self::Http(e)
    }
}

#[derive(Debug, Deserialize)]
struct ESearchResult {
    esearchresult: ESearchInner,
}

#[derive(Debug, Deserialize)]
struct ESearchInner {
    count: String,
    #[serde(default)]
    idlist: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ESpellResult {
    esearchresult: ESpellInner,
}

#[derive(Debug, Deserialize)]
struct ESpellInner {
    #[serde(default)]
    correctedquery: Option<String>,
}

fn parse_pubmed_xml(xml: &str) -> Result<Vec<Paper>, PubMedError> {
    let doc = roxmltree::Document::parse(xml)
        .map_err(|e| PubMedError::Parse(format!("XML parse: {e}")))?;

    let mut papers = Vec::new();
    for article in doc.descendants().filter(|n| n.has_tag_name("PubmedArticle")) {
        if let Some(paper) = parse_pubmed_article(&doc, article) {
            papers.push(paper);
        }
    }
    Ok(papers)
}

fn parse_pubmed_article(_doc: &roxmltree::Document, article: roxmltree::Node) -> Option<Paper> {
    let medline = article
        .descendants()
        .find(|n| n.has_tag_name("MedlineCitation"))?;

    let pmid = medline
        .descendants()
        .find(|n| n.has_tag_name("PMID"))
        .and_then(|n| n.text())
        .unwrap_or("")
        .to_string();

    let article_node = medline
        .descendants()
        .find(|n| n.has_tag_name("Article"))?;

    let title = article_node
        .descendants()
        .find(|n| n.has_tag_name("ArticleTitle"))
        .and_then(|n| n.text())
        .unwrap_or("")
        .to_string();

    let abstract_text = article_node
        .descendants()
        .find(|n| n.has_tag_name("AbstractText"))
        .and_then(|n| n.text())
        .map(|s| s.to_string());

    let journal = article_node
        .descendants()
        .find(|n| n.has_tag_name("Title"))
        .and_then(|n| n.text())
        .map(|s| s.to_string());

    let year = article_node
        .descendants()
        .find(|n| n.has_tag_name("Year"))
        .and_then(|n| n.text())
        .map(|s| s.to_string());

    let volume = article_node
        .descendants()
        .find(|n| n.has_tag_name("Volume"))
        .and_then(|n| n.text())
        .map(|s| s.to_string());

    let issue = article_node
        .descendants()
        .find(|n| n.has_tag_name("Issue"))
        .and_then(|n| n.text())
        .map(|s| s.to_string());

    let pages = article_node
        .descendants()
        .find(|n| n.has_tag_name("MedlinePgn"))
        .and_then(|n| n.text())
        .map(|s| s.to_string());

    let doi = article_node
        .descendants()
        .find(|n| n.has_tag_name("ELocationID") && n.attribute("EIdType") == Some("doi"))
        .and_then(|n| n.text())
        .map(|s| s.to_string());

    let authors: Vec<Author> = article_node
        .descendants()
        .filter(|n| n.has_tag_name("Author") && n.parent().map_or(false, |p| p.has_tag_name("AuthorList")))
        .filter_map(|author_node| {
            let last_name = author_node
                .descendants()
                .find(|n| n.has_tag_name("LastName"))
                .and_then(|n| n.text())
                .unwrap_or("")
                .to_string();
            let fore_name = author_node
                .descendants()
                .find(|n| n.has_tag_name("ForeName"))
                .and_then(|n| n.text())
                .map(|s| s.to_string());
            let initials = author_node
                .descendants()
                .find(|n| n.has_tag_name("Initials"))
                .and_then(|n| n.text())
                .map(|s| s.to_string());

            Some(Author {
                last_name,
                fore_name,
                initials,
                affiliation: None,
            })
        })
        .collect();

    let mesh_terms: Vec<MeSHTerm> = medline
        .descendants()
        .filter(|n| n.has_tag_name("MeshHeading"))
        .map(|mesh_node| {
            let descriptor = mesh_node
                .descendants()
                .find(|n| n.has_tag_name("DescriptorName"))
                .and_then(|n| n.text())
                .unwrap_or("")
                .to_string();
            let is_major = mesh_node
                .descendants()
                .find(|n| n.has_tag_name("DescriptorName"))
                .and_then(|n| n.attribute("MajorTopicYN"))
                .map(|a| a == "Y")
                .unwrap_or(false);
            let qualifier = mesh_node
                .descendants()
                .find(|n| n.has_tag_name("QualifierName"))
                .and_then(|n| n.text())
                .map(|s| s.to_string());

            MeSHTerm {
                descriptor,
                qualifier,
                is_major,
            }
        })
        .collect();

    let publication_types: Vec<String> = article_node
        .descendants()
        .filter(|n| n.has_tag_name("PublicationType"))
        .filter_map(|n| n.text().map(|s| s.to_string()))
        .collect();

    let pmcid = article_node
        .descendants()
        .find(|n| n.has_tag_name("ArticleId") && n.attribute("IdType") == Some("pmc"))
        .and_then(|n| n.text())
        .map(|s| s.to_string());

    Some(Paper {
        pmid,
        title,
        abstract_text,
        authors,
        journal,
        year,
        volume,
        issue,
        pages,
        doi,
        pmcid,
        mesh_terms,
        publication_types,
    })
}

fn parse_pmc_fulltext(xml: &str) -> Result<Vec<FullText>, PubMedError> {
    let doc = roxmltree::Document::parse(xml)
        .map_err(|e| PubMedError::Parse(format!("PMC XML parse: {e}")))?;

    let mut fulltexts = Vec::new();
    for article in doc.descendants().filter(|n| n.has_tag_name("article")) {
        let pmid = article
            .descendants()
            .find(|n| n.has_tag_name("article-id") && n.attribute("pub-id-type") == Some("pmid"))
            .and_then(|n| n.text())
            .unwrap_or("")
            .to_string();

        let title = article
            .descendants()
            .find(|n| n.has_tag_name("article-title"))
            .and_then(|n| n.text())
            .unwrap_or("")
            .to_string();

        let sections: Vec<FullTextSection> = article
            .descendants()
            .filter(|n| n.has_tag_name("sec"))
            .filter_map(|sec_node| {
                let sec_title = sec_node
                    .descendants()
                    .find(|n| n.has_tag_name("title"))
                    .and_then(|n| n.text())
                    .map(|s| s.to_string());

                let content: String = sec_node
                    .descendants()
                    .filter(|n| n.has_tag_name("p"))
                    .filter_map(|p| p.text())
                    .collect::<Vec<_>>()
                    .join("\n");

                if content.is_empty() {
                    return None;
                }

                Some(FullTextSection {
                    title: sec_title,
                    content,
                })
            })
            .collect();

        fulltexts.push(FullText {
            pmid,
            title,
            sections,
            references: Vec::new(),
            source: "pmc".to_string(),
        });
    }

    Ok(fulltexts)
}
