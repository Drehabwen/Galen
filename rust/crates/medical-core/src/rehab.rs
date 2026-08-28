//! Rehabilitation-specific literature search support.
//!
//! Builds PubMed queries that combine rehabilitation MeSH terms, sub-specialty
//! focus areas and study-design filters — so the model never has to know
//! PubMed query syntax to get high-quality rehab evidence.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Focus areas
// ---------------------------------------------------------------------------

/// Rehabilitation sub-specialty focus areas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RehabFocus {
    /// Stroke / acquired brain injury / Parkinson — neurorehabilitation
    Neuro,
    /// Joint replacement, ACL, fracture, OA — orthopedic & post-surgical rehab
    Ortho,
    /// Cerebral palsy, developmental disorders — pediatric rehab
    Pediatric,
    /// Heart failure, COPD — cardiac & pulmonary rehab
    CardioPulmonary,
    /// Spinal cord injury
    Spinal,
}

impl RehabFocus {
    /// Stable machine id (used by tool schemas).
    pub fn id(self) -> &'static str {
        match self {
            RehabFocus::Neuro => "neuro",
            RehabFocus::Ortho => "ortho",
            RehabFocus::Pediatric => "pediatric",
            RehabFocus::CardioPulmonary => "cardiopulmonary",
            RehabFocus::Spinal => "spinal",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            RehabFocus::Neuro => "神经康复",
            RehabFocus::Ortho => "骨科/术后康复",
            RehabFocus::Pediatric => "儿童康复",
            RehabFocus::CardioPulmonary => "心肺康复",
            RehabFocus::Spinal => "脊髓损伤康复",
        }
    }

    pub fn from_id(s: &str) -> Option<Self> {
        Self::all().into_iter().find(|f| f.id() == s).copied()
    }

    pub fn all() -> &'static [RehabFocus] {
        &[
            RehabFocus::Neuro,
            RehabFocus::Ortho,
            RehabFocus::Pediatric,
            RehabFocus::CardioPulmonary,
            RehabFocus::Spinal,
        ]
    }

    /// MeSH descriptors that narrow a search to this focus.
    fn mesh_terms(self) -> &'static [&'static str] {
        match self {
            RehabFocus::Neuro => &[
                "stroke",
                "brain injuries",
                "parkinson disease",
                "multiple sclerosis",
            ],
            RehabFocus::Ortho => &[
                "arthroplasty",
                "anterior cruciate ligament",
                "fractures",
                "osteoarthritis",
                "low back pain",
            ],
            RehabFocus::Pediatric => &[
                "cerebral palsy",
                "child development disorders",
                "musculoskeletal abnormalities",
            ],
            RehabFocus::CardioPulmonary => &[
                "heart failure",
                "pulmonary disease, chronic obstructive",
                "cardiac rehabilitation",
                "myocardial infarction",
            ],
            RehabFocus::Spinal => &["spinal cord injuries", "spinal cord diseases"],
        }
    }
}

// ---------------------------------------------------------------------------
// Study design filter
// ---------------------------------------------------------------------------

/// Evidence-level filter applied to rehab searches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RehabStudyType {
    Any,
    Rct,
    SystematicReview,
    MetaAnalysis,
}

impl RehabStudyType {
    pub fn id(self) -> &'static str {
        match self {
            RehabStudyType::Any => "any",
            RehabStudyType::Rct => "rct",
            RehabStudyType::SystematicReview => "systematic_review",
            RehabStudyType::MetaAnalysis => "meta_analysis",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            RehabStudyType::Any => "不限研究类型",
            RehabStudyType::Rct => "随机对照试验 (RCT)",
            RehabStudyType::SystematicReview => "系统评价",
            RehabStudyType::MetaAnalysis => "Meta 分析",
        }
    }

    pub fn from_id(s: &str) -> Self {
        match s {
            "rct" => RehabStudyType::Rct,
            "systematic_review" => RehabStudyType::SystematicReview,
            "meta_analysis" => RehabStudyType::MetaAnalysis,
            _ => RehabStudyType::Any,
        }
    }

    /// PubMed publication-type filter clause, if any.
    fn filter_clause(self) -> Option<&'static str> {
        match self {
            RehabStudyType::Any => None,
            RehabStudyType::Rct => Some(r#""randomized controlled trial"[pt]"#),
            RehabStudyType::SystematicReview => Some(r#""systematic review"[pt]"#),
            RehabStudyType::MetaAnalysis => Some(r#""meta-analysis"[pt]"#),
        }
    }
}

// ---------------------------------------------------------------------------
// Query builder
// ---------------------------------------------------------------------------

/// Build a PubMed query string for a rehabilitation topic.
///
/// Combines the user's topic terms, the `rehabilitation[MeSH]` umbrella,
/// focus-area MeSH descriptors (OR-grouped) and an optional study-type filter.
///
/// # Example
///
/// ```
/// use medical_core::rehab::{build_rehab_query, RehabFocus, RehabStudyType};
/// let q = build_rehab_query("trunk control after stroke", Some(RehabFocus::Neuro), RehabStudyType::Rct);
/// assert!(q.contains("rehabilitation[MeSH]"));
/// assert!(q.contains(r#""randomized controlled trial"[pt]"#));
/// ```
pub fn build_rehab_query(
    topic: &str,
    focus: Option<RehabFocus>,
    study_type: RehabStudyType,
) -> String {
    let mut parts: Vec<String> = Vec::with_capacity(4);

    let topic = topic.trim();
    if !topic.is_empty() {
        parts.push(topic.to_string());
    }

    parts.push("rehabilitation[MeSH]".to_string());

    if let Some(f) = focus {
        let group = f
            .mesh_terms()
            .iter()
            .map(|t| format!("{t}[MeSH]"))
            .collect::<Vec<_>>()
            .join(" OR ");
        parts.push(format!("({group})"));
    }

    if let Some(clause) = study_type.filter_clause() {
        parts.push(clause.to_string());
    }

    parts.join(" AND ")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_contains_rehab_umbrella_and_topic() {
        let q = build_rehab_query("trunk control", None, RehabStudyType::Any);
        assert!(q.contains("trunk control"));
        assert!(q.contains("rehabilitation[MeSH]"));
    }

    #[test]
    fn focus_terms_are_or_grouped() {
        let q = build_rehab_query("gait", Some(RehabFocus::Neuro), RehabStudyType::Any);
        assert!(q.contains("(stroke[MeSH] OR brain injuries[MeSH]"));
        assert!(q.contains(")"));
    }

    #[test]
    fn study_type_appends_pt_filter() {
        let q = build_rehab_query("balance", None, RehabStudyType::MetaAnalysis);
        assert!(q.contains(r#""meta-analysis"[pt]"#));

        let any = build_rehab_query("balance", None, RehabStudyType::Any);
        assert!(!any.contains("[pt]"));
    }

    #[test]
    fn empty_topic_yields_valid_query() {
        let q = build_rehab_query("   ", None, RehabStudyType::Rct);
        assert!(!q.starts_with("AND"));
        assert!(q.contains("rehabilitation[MeSH]"));
    }

    #[test]
    fn focus_ids_roundtrip() {
        for f in RehabFocus::all() {
            assert_eq!(RehabFocus::from_id(f.id()), Some(*f));
        }
        assert_eq!(RehabFocus::from_id("bogus"), None);
    }

    #[test]
    fn study_type_ids_roundtrip() {
        assert_eq!(RehabStudyType::from_id("rct"), RehabStudyType::Rct);
        assert_eq!(RehabStudyType::from_id("nope"), RehabStudyType::Any);
    }
}
