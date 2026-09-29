use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

const REGISTRY_JSON: &str = include_str!("../protocols/rehab-screening-core-v1.json");

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    Observed,
    Derived,
    ModelEstimate,
    HumanJudgment,
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AllowedUse {
    SpecialtyEvidence,
    ScreeningEvidence,
    ResearchOnly,
    Excluded,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MetricProtocol {
    pub canonical_metric: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub prefixes: Vec<String>,
    pub unit: Option<String>,
    pub evidence_kind: EvidenceKind,
    pub allowed_use: AllowedUse,
    pub requires_human_review: bool,
    pub directional: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolRegistry {
    pub schema_version: u32,
    pub registry_id: String,
    pub version: String,
    pub title: String,
    pub scope: String,
    pub metrics: Vec<MetricProtocol>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolResolution {
    pub registry_id: String,
    pub registry_version: String,
    pub canonical_metric: String,
    pub expected_unit: Option<String>,
    pub evidence_kind: EvidenceKind,
    pub allowed_use: AllowedUse,
    pub requires_human_review: bool,
    pub directional: bool,
    pub registered: bool,
    pub unit_matches: bool,
}

pub fn registry() -> Result<&'static ProtocolRegistry, String> {
    static REGISTRY: OnceLock<Result<ProtocolRegistry, String>> = OnceLock::new();
    REGISTRY
        .get_or_init(|| {
            let registry: ProtocolRegistry = serde_json::from_str(REGISTRY_JSON)
                .map_err(|error| format!("康复协议注册表格式无效: {error}"))?;
            validate_registry(&registry)?;
            Ok(registry)
        })
        .as_ref()
        .map_err(Clone::clone)
}

pub fn resolve_metric(metric: &str, unit: &str) -> Result<ProtocolResolution, String> {
    let registry = registry()?;
    let normalized_metric = metric.trim();
    let lower_metric = normalized_metric.to_ascii_lowercase();
    let matched = registry.metrics.iter().find(|definition| {
        definition
            .aliases
            .iter()
            .any(|alias| alias.eq_ignore_ascii_case(normalized_metric))
            || definition
                .prefixes
                .iter()
                .any(|prefix| lower_metric.starts_with(&prefix.to_ascii_lowercase()))
    });
    let Some(definition) = matched else {
        return Ok(ProtocolResolution {
            registry_id: registry.registry_id.clone(),
            registry_version: registry.version.clone(),
            canonical_metric: normalized_metric.to_string(),
            expected_unit: None,
            evidence_kind: EvidenceKind::Unknown,
            allowed_use: AllowedUse::ResearchOnly,
            requires_human_review: true,
            directional: false,
            registered: false,
            unit_matches: true,
        });
    };
    let exact_alias = definition
        .aliases
        .iter()
        .any(|alias| alias.eq_ignore_ascii_case(normalized_metric));
    let canonical_metric = if exact_alias {
        definition.canonical_metric.clone()
    } else {
        normalized_metric.to_string()
    };
    Ok(ProtocolResolution {
        registry_id: registry.registry_id.clone(),
        registry_version: registry.version.clone(),
        canonical_metric,
        expected_unit: definition.unit.clone(),
        evidence_kind: definition.evidence_kind,
        allowed_use: definition.allowed_use,
        requires_human_review: definition.requires_human_review,
        directional: definition.directional,
        registered: true,
        unit_matches: definition
            .unit
            .as_deref()
            .map(|expected| units_equivalent(expected, unit))
            .unwrap_or(true),
    })
}

fn validate_registry(registry: &ProtocolRegistry) -> Result<(), String> {
    if registry.schema_version != 1 {
        return Err("康复协议注册表仅支持 schemaVersion=1".into());
    }
    if registry.registry_id.trim().is_empty() || registry.version.trim().is_empty() {
        return Err("康复协议注册表缺少 ID 或版本".into());
    }
    if registry.metrics.is_empty() {
        return Err("康复协议注册表不能没有指标".into());
    }
    let mut selectors = std::collections::BTreeSet::new();
    for metric in &registry.metrics {
        if metric.canonical_metric.trim().is_empty()
            || (metric.aliases.is_empty() && metric.prefixes.is_empty())
        {
            return Err("康复协议指标缺少规范名或匹配规则".into());
        }
        for selector in metric.aliases.iter().chain(metric.prefixes.iter()) {
            if !selectors.insert(selector.to_ascii_lowercase()) {
                return Err(format!("康复协议匹配规则重复: {selector}"));
            }
        }
    }
    Ok(())
}

fn units_equivalent(expected: &str, actual: &str) -> bool {
    fn normalized(value: &str) -> String {
        match value.trim().to_ascii_lowercase().as_str() {
            "degree" | "degrees" | "°" => "deg".into(),
            value => value.into(),
        }
    }
    normalized(expected) == normalized(actual)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_is_versioned_and_valid() {
        let registry = registry().unwrap();
        assert_eq!(registry.registry_id, "rehab-screening-core");
        assert_eq!(registry.version, "1.0.0");
    }

    #[test]
    fn resolves_alias_and_normalizes_canonical_metric() {
        let resolved = resolve_metric("adams_atrDegrees", "degrees").unwrap();
        assert_eq!(resolved.canonical_metric, "adams_atr_deg");
        assert!(resolved.unit_matches);
        assert_eq!(resolved.allowed_use, AllowedUse::SpecialtyEvidence);
    }

    #[test]
    fn flags_unit_mismatch_and_excluded_estimate() {
        assert!(
            !resolve_metric("rom_knee_flexion_left", "cm")
                .unwrap()
                .unit_matches
        );
        assert_eq!(
            resolve_metric("adams_cobbAngleEstimate", "deg")
                .unwrap()
                .allowed_use,
            AllowedUse::Excluded
        );
    }

    #[test]
    fn unknown_metric_remains_research_only_and_reviewable() {
        let resolved = resolve_metric("novel_sensor_index", "value").unwrap();
        assert!(!resolved.registered);
        assert!(resolved.requires_human_review);
        assert_eq!(resolved.allowed_use, AllowedUse::ResearchOnly);
    }
}
