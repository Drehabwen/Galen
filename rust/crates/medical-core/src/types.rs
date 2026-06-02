use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Paper {
    pub pmid: String,
    pub title: String,
    pub abstract_text: Option<String>,
    pub authors: Vec<Author>,
    pub journal: Option<String>,
    pub year: Option<String>,
    pub volume: Option<String>,
    pub issue: Option<String>,
    pub pages: Option<String>,
    pub doi: Option<String>,
    pub pmcid: Option<String>,
    pub mesh_terms: Vec<MeSHTerm>,
    pub publication_types: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Author {
    pub last_name: String,
    pub fore_name: Option<String>,
    pub initials: Option<String>,
    pub affiliation: Option<String>,
}

impl std::fmt::Display for Author {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.fore_name.as_deref() {
            Some(fore) if !fore.is_empty() => write!(f,"{}, {}", self.last_name, fore),
            _ => write!(f, "{}", self.last_name),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeSHTerm {
    pub descriptor: String,
    pub qualifier: Option<String>,
    pub is_major: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub total_count: u64,
    pub pmids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullTextSection {
    pub title: Option<String>,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullText {
    pub pmid: String,
    pub title: String,
    pub sections: Vec<FullTextSection>,
    pub references: Vec<Citation>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Citation {
    pub pmid: Option<String>,
    pub title: Option<String>,
    pub authors: Option<String>,
    pub journal: Option<String>,
    pub year: Option<String>,
    pub volume: Option<String>,
    pub pages: Option<String>,
    pub doi: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CitationStyle {
    Apa,
    Vancouver,
    BibTeX,
    RIS,
    Mla,
}

impl CitationStyle {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "apa" => Some(Self::Apa),
            "vancouver" => Some(Self::Vancouver),
            "bibtex" | "bib" => Some(Self::BibTeX),
            "ris" => Some(Self::RIS),
            "mla" => Some(Self::Mla),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchParams {
    pub query: String,
    pub max_results: u32,
    pub sort: Option<String>,
    pub min_date: Option<String>,
    pub max_date: Option<String>,
    pub has_abstract: Option<bool>,
    pub free_full_text: Option<bool>,
    pub language: Option<String>,
    pub publication_types: Option<Vec<String>>,
}

impl Default for SearchParams {
    fn default() -> Self {
        Self {
            query: String::new(),
            max_results: 20,
            sort: Some("relevance".into()),
            min_date: None,
            max_date: None,
            has_abstract: None,
            free_full_text: None,
            language: None,
            publication_types: None,
        }
    }
}
