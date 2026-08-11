use crate::types::{CitationStyle, Paper};

pub fn format_citations(papers: &[Paper], style: CitationStyle) -> String {
    match style {
        CitationStyle::Apa => format_apa(papers),
        CitationStyle::Vancouver => format_vancouver(papers),
        CitationStyle::BibTeX => format_bibtex(papers),
        CitationStyle::RIS => format_ris(papers),
        CitationStyle::Mla => format_mla(papers),
    }
}

pub fn format_single(paper: &Paper, style: CitationStyle) -> String {
    format_citations(&[paper.clone()], style)
}

fn format_apa(papers: &[Paper]) -> String {
    papers
        .iter()
        .enumerate()
        .map(|(i, p)| format!("{}. {}", i + 1, apa_single(p)))
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn apa_single(p: &Paper) -> String {
    let authors = format_apa_authors(&p.authors);
    let year = p.year.as_deref().unwrap_or("n.d.");
    let title = &p.title;
    let journal = p.journal.as_deref().unwrap_or("[Unknown Journal]");
    let volume = p.volume.as_deref().unwrap_or("");
    let issue = p.issue.as_deref().unwrap_or("");
    let pages = p.pages.as_deref().unwrap_or("");
    let doi = p.doi.as_deref().unwrap_or("");

    let mut citation = format!("{authors} ({year}). {title}. *{journal}*");

    if !volume.is_empty() {
        citation.push_str(&format!(", *{volume}*"));
        if !issue.is_empty() {
            citation.push_str(&format!("({issue})"));
        }
    }
    if !pages.is_empty() {
        citation.push_str(&format!(", {pages}"));
    }
    if !doi.is_empty() {
        citation.push_str(&format!(". https://doi.org/{doi}"));
    } else {
        citation.push('.');
    }

    citation
}

fn format_apa_authors(authors: &[crate::types::Author]) -> String {
    match authors.len() {
        0 => "[Unknown Author]".to_string(),
        1 => authors[0].to_string(),
        2 => format!("{} & {}", authors[0].to_string(), authors[1].to_string()),
        _ if authors.len() <= 7 => {
            let names: Vec<String> = authors
                .iter()
                .map(|a| a.to_string())
                .collect();
            let last = names.len() - 1;
            format!(
                "{} & {}",
                names[..last].join(", "),
                names[last]
            )
        }
        _ => {
            let first_six: Vec<String> = authors
                .iter()
                .take(6)
                .map(|a| a.to_string())
                .collect();
            format!(
                "{} ... {}",
                first_six.join(", "),
                authors.last().unwrap().to_string()
            )
        }
    }
}

fn format_vancouver(papers: &[Paper]) -> String {
    papers
        .iter()
        .enumerate()
        .map(|(i, p)| format!("{}. {}", i + 1, vancouver_single(p)))
        .collect::<Vec<_>>()
        .join("\n")
}

fn vancouver_single(p: &Paper) -> String {
    let authors = format_vancouver_authors(&p.authors);
    let title = &p.title;
    let journal = p
        .journal
        .as_deref()
        .map(|j| abbreviate_journal(j))
        .unwrap_or_else(|| "[Unknown Journal]".to_string());
    let year = p.year.as_deref().unwrap_or("n.d.");
    let volume = p.volume.as_deref().unwrap_or("");
    let issue = p.issue.as_deref().unwrap_or("");
    let pages = p.pages.as_deref().unwrap_or("");
    let doi = p.doi.as_deref().unwrap_or("");

    let mut citation = format!("{authors}. {title}. {journal}. {year}");

    if !volume.is_empty() {
        citation.push_str(&format!(";{volume}"));
        if !issue.is_empty() {
            citation.push_str(&format!("({issue})"));
        }
    }
    if !pages.is_empty() {
        citation.push_str(&format!(":{pages}"));
    }
    if !doi.is_empty() {
        citation.push_str(&format!(". doi:{doi}"));
    }

    citation
}

fn format_vancouver_authors(authors: &[crate::types::Author]) -> String {
    match authors.len() {
        0 => "[Unknown Author]".to_string(),
        _ if authors.len() <= 6 => {
            let names: Vec<String> = authors
                .iter()
                .map(|a| format!("{} {}", a.last_name, a.initials.as_deref().unwrap_or("")))
                .collect();
            names.join(", ")
        }
        _ => {
            let first_three: Vec<String> = authors
                .iter()
                .take(3)
                .map(|a| format!("{} {}", a.last_name, a.initials.as_deref().unwrap_or("")))
                .collect();
            format!("{}, et al.", first_three.join(", "))
        }
    }
}

fn abbreviate_journal(journal: &str) -> String {
    journal
        .split_whitespace()
        .map(|w| {
            if w.len() <= 3 || w.to_uppercase() == w {
                w.to_string()
            } else {
                let cleaned: String = w
                    .chars()
                    .filter(|c| c.is_alphabetic())
                    .collect();
                if cleaned.len() <= 3 {
                    cleaned
                } else {
                    format!("{}.", cleaned)
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn format_bibtex(papers: &[Paper]) -> String {
    papers
        .iter()
        .map(bibtex_single)
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn bibtex_single(p: &Paper) -> String {
    let key = bibtex_key(p);
    let year = p.year.as_deref().unwrap_or("0000");

    let mut entry = format!("@article{{{key},\n");
    entry.push_str(&format!("  author = {{{}}},\n", format_bibtex_authors(&p.authors)));
    entry.push_str(&format!("  title = {{{}}},\n", p.title));
    if let Some(ref journal) = p.journal {
        entry.push_str(&format!("  journal = {{{journal}}},\n"));
    }
    entry.push_str(&format!("  year = {{{year}}},\n"));
    if let Some(ref vol) = p.volume {
        entry.push_str(&format!("  volume = {{{vol}}},\n"));
    }
    if let Some(ref issue) = p.issue {
        entry.push_str(&format!("  number = {{{issue}}},\n"));
    }
    if let Some(ref pages) = p.pages {
        entry.push_str(&format!("  pages = {{{pages}}},\n"));
    }
    if let Some(ref doi) = p.doi {
        entry.push_str(&format!("  doi = {{{doi}}},\n"));
    }
    if let Some(ref pmid) = p.pmid.split_whitespace().next() {
        if !pmid.is_empty() {
            entry.push_str(&format!("  pmid = {{{pmid}}},\n"));
        }
    }
    entry.push('}');
    entry
}

fn bibtex_key(p: &Paper) -> String {
    let first_author = p
        .authors
        .first()
        .map(|a| a.last_name.to_lowercase())
        .unwrap_or_else(|| "unknown".to_string());
    let year = p.year.as_deref().unwrap_or("0000");
    let first_word = p
        .title
        .split_whitespace()
        .next()
        .map(|w| w.to_lowercase())
        .unwrap_or_else(|| "untitled".to_string());
    format!("{first_author}{year}{first_word}")
}

fn format_bibtex_authors(authors: &[crate::types::Author]) -> String {
    if authors.is_empty() {
        return "Unknown".to_string();
    }
    let names: Vec<String> = authors
        .iter()
        .map(|a| {
            format!(
                "{} {}",
                a.last_name,
                a.initials.as_deref().unwrap_or("")
            )
            .trim()
            .to_string()
        })
        .collect();
    names.join(" and ")
}

fn format_ris(papers: &[Paper]) -> String {
    papers.iter().map(ris_single).collect::<Vec<_>>().join("\n\n")
}

fn ris_single(p: &Paper) -> String {
    let mut entry = String::from("TY  - JOUR\n");
    entry.push_str(&format!("TI  - {}\n", p.title));

    for author in &p.authors {
        entry.push_str(&format!(
            "AU  - {}\n",
            format!("{} {}", author.last_name, author.fore_name.as_deref().unwrap_or(""))
        ));
    }

    if let Some(ref journal) = p.journal {
        entry.push_str(&format!("JO  - {journal}\n"));
    }
    if let Some(ref year) = p.year {
        entry.push_str(&format!("PY  - {year}\n"));
    }
    if let Some(ref vol) = p.volume {
        entry.push_str(&format!("VL  - {vol}\n"));
    }
    if let Some(ref issue) = p.issue {
        entry.push_str(&format!("IS  - {issue}\n"));
    }
    if let Some(ref pages) = p.pages {
        entry.push_str(&format!("SP  - {pages}\n"));
    }
    if let Some(ref doi) = p.doi {
        entry.push_str(&format!("DO  - {doi}\n"));
    }
    if !p.pmid.is_empty() {
        entry.push_str(&format!("AN  - {}\n", p.pmid));
    }
    entry.push_str("ER  - ");
    entry
}

fn format_mla(papers: &[Paper]) -> String {
    papers
        .iter()
        .enumerate()
        .map(|(i, p)| format!("{}. {}", i + 1, mla_single(p)))
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn mla_single(p: &Paper) -> String {
    let authors = format_mla_authors(&p.authors);
    let title = &p.title;
    let journal = p.journal.as_deref().unwrap_or("[Unknown Journal]");
    let volume = p.volume.as_deref().unwrap_or("");
    let issue = p.issue.as_deref().unwrap_or("");
    let year = p.year.as_deref().unwrap_or("n.d.");
    let pages = p.pages.as_deref().unwrap_or("");
    let doi = p.doi.as_deref().unwrap_or("");

    let mut citation = format!("{authors}. \"{title}.\" *{journal}*");
    if !volume.is_empty() {
        citation.push_str(&format!(", vol. {volume}"));
        if !issue.is_empty() {
            citation.push_str(&format!(", no. {issue}"));
        }
    }
    citation.push_str(&format!(", {year}"));
    if !pages.is_empty() {
        citation.push_str(&format!(", pp. {pages}"));
    }
    if !doi.is_empty() {
        citation.push_str(&format!(". doi:{doi}"));
    } else {
        citation.push('.');
    }

    citation
}

fn format_mla_authors(authors: &[crate::types::Author]) -> String {
    match authors.len() {
        0 => "[Unknown Author]".to_string(),
        1 => authors[0].to_string(),
        2 => format!("{}, and {}", authors[0].to_string(), authors[1].to_string()),
        _ => format!("{}, et al.", authors[0].to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Author;

    fn sample_paper() -> Paper {
        Paper {
            pmid: "12345678".into(),
            title: "Metformin mechanisms in diabetes treatment".into(),
            abstract_text: Some("Metformin is a first-line medication...".into()),
            authors: vec![
                Author {
                    last_name: "Smith".into(),
                    fore_name: Some("John".into()),
                    initials: Some("J".into()),
                    affiliation: None,
                },
                Author {
                    last_name: "Jones".into(),
                    fore_name: Some("Mary".into()),
                    initials: Some("M".into()),
                    affiliation: None,
                },
            ],
            journal: Some("Nature Medicine".into()),
            year: Some("2024".into()),
            volume: Some("30".into()),
            issue: Some("2".into()),
            pages: Some("123-130".into()),
            doi: Some("10.1038/s41591-024-00123-4".into()),
            pmcid: None,
            mesh_terms: Vec::new(),
            publication_types: Vec::new(),
        }
    }

    #[test]
    fn formats_apa() {
        let result = format_single(&sample_paper(), CitationStyle::Apa);
        assert!(result.contains("Smith, John & Jones, Mary"));
        assert!(result.contains("(2024)"));
        assert!(result.contains("Nature Medicine"));
    }

    #[test]
    fn formats_vancouver() {
        let result = format_single(&sample_paper(), CitationStyle::Vancouver);
        assert!(result.contains("Smith J, Jones M"));
        assert!(result.contains("2024"));
    }

    #[test]
    fn formats_bibtex() {
        let result = format_single(&sample_paper(), CitationStyle::BibTeX);
        assert!(result.contains("@article{"));
        assert!(result.contains("author = {Smith J and Jones M}"));
        assert!(result.contains("title = {Metformin mechanisms"));
    }

    #[test]
    fn formats_ris() {
        let result = format_single(&sample_paper(), CitationStyle::RIS);
        assert!(result.contains("TY  - JOUR"));
        assert!(result.contains("ER  - "));
    }

    #[test]
    fn formats_mla() {
        let result = format_single(&sample_paper(), CitationStyle::Mla);
        assert!(result.contains("Smith, John"));
        assert!(result.contains("Jones, Mary"));
        assert!(result.contains("vol. 30"));
    }

    #[test]
    fn citation_style_from_str() {
        assert_eq!(CitationStyle::from_str("apa"), Some(CitationStyle::Apa));
        assert_eq!(CitationStyle::from_str("VANCOUVER"), Some(CitationStyle::Vancouver));
        assert_eq!(CitationStyle::from_str("bibtex"), Some(CitationStyle::BibTeX));
        assert_eq!(CitationStyle::from_str("RIS"), Some(CitationStyle::RIS));
        assert_eq!(CitationStyle::from_str("mla"), Some(CitationStyle::Mla));
        assert_eq!(CitationStyle::from_str("unknown"), None);
    }
}
