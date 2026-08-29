//! Capability packs keep domain workflows outside the Galen kernel.
//!
//! A pack declares its identity and registers tools through stable kernel
//! primitives. The default workbench is assembled from packs, while callers
//! can still construct a kernel-only registry for tests or custom products.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::tools::ToolRegistry;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityLayer {
    Kernel,
    Workbench,
    Domain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityUiSlot {
    TopBar,
    ResourceBar,
    Inspector,
    Settings,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityManifest {
    pub id: &'static str,
    pub name: &'static str,
    pub version: &'static str,
    pub layer: CapabilityLayer,
    pub description: &'static str,
    pub tool_names: &'static [&'static str],
    pub ui_slots: &'static [CapabilityUiSlot],
    pub context_modules: &'static [&'static str],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityStatus {
    #[serde(flatten)]
    pub manifest: CapabilityManifest,
    pub enabled: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CapabilityConfig {
    /// Omit this field to enable every official pack.
    pub enabled: Option<Vec<String>>,
}

impl CapabilityConfig {
    pub fn load() -> Self {
        let Some(path) = config_path() else {
            return Self::default();
        };
        std::fs::read_to_string(path)
            .ok()
            .and_then(|content| toml::from_str(&content).ok())
            .unwrap_or_default()
    }

    pub fn enables(&self, id: &str) -> bool {
        self.enabled
            .as_ref()
            .is_none_or(|ids| ids.iter().any(|enabled| enabled == id))
    }
}

fn config_path() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".galen").join("capabilities.toml"))
}

pub trait CapabilityPack: Send + Sync {
    fn manifest(&self) -> CapabilityManifest;
    fn register(&self, registry: &mut ToolRegistry);
}

#[derive(Default)]
pub struct CapabilityRegistry {
    manifests: Vec<CapabilityManifest>,
}

impl CapabilityRegistry {
    pub fn activate(&mut self, pack: impl CapabilityPack, tool_registry: &mut ToolRegistry) {
        let manifest = pack.manifest();
        if self.manifests.iter().any(|item| item.id == manifest.id) {
            return;
        }
        pack.register(tool_registry);
        self.manifests.push(manifest);
    }

    pub fn manifests(&self) -> &[CapabilityManifest] {
        &self.manifests
    }
}

pub mod packs {
    use super::{CapabilityLayer, CapabilityManifest, CapabilityPack, CapabilityUiSlot};
    use crate::tools::{self, ToolRegistry};

    pub struct ResearchPack;

    impl CapabilityPack for ResearchPack {
        fn manifest(&self) -> CapabilityManifest {
            CapabilityManifest {
                id: "galen.research",
                name: "Research Workbench",
                version: "0.1.0",
                layer: CapabilityLayer::Workbench,
                description: "Research planning, evidence search, and literature tools.",
                tool_names: &[
                    "create_research_plan",
                    "search_evidence",
                    "search_pubmed",
                    "fetch_article",
                    "format_citation",
                    "search_rehab_literature",
                ],
                ui_slots: &[CapabilityUiSlot::TopBar, CapabilityUiSlot::Inspector],
                context_modules: &["research_plan", "evidence_chain"],
            }
        }

        fn register(&self, registry: &mut ToolRegistry) {
            registry.register(tools::research::CreateResearchPlan);
            registry.register(tools::evidence_search::SearchEvidence);
            registry.register(tools::medical::SearchPubMed);
            registry.register(tools::medical::FetchArticle);
            registry.register(tools::medical::FormatCitation);
            registry.register(tools::medical::SearchRehabLiterature);
        }
    }

    pub struct RehabilitationPack;

    impl CapabilityPack for RehabilitationPack {
        fn manifest(&self) -> CapabilityManifest {
            CapabilityManifest {
                id: "galen.rehabilitation",
                name: "Rehabilitation Analysis",
                version: "0.1.0",
                layer: CapabilityLayer::Domain,
                description: "Clinical case and rehabilitation data analysis tools.",
                tool_names: &["analyze_clinical_case", "rehab_data"],
                ui_slots: &[CapabilityUiSlot::Inspector],
                context_modules: &["rehabilitation_context"],
            }
        }

        fn register(&self, registry: &mut ToolRegistry) {
            registry.register(tools::clinical::AnalyzeClinicalCase);
            registry.register(tools::rehab::RehabData);
        }
    }

    pub struct PdfReportPack;

    impl CapabilityPack for PdfReportPack {
        fn manifest(&self) -> CapabilityManifest {
            CapabilityManifest {
                id: "galen.pdf-report",
                name: "PDF Report",
                version: "0.1.0",
                layer: CapabilityLayer::Domain,
                description: "Compile Typst sources into registered PDF artifacts.",
                tool_names: &["compile_pdf_report"],
                ui_slots: &[CapabilityUiSlot::ResourceBar],
                context_modules: &["artifact_delivery"],
            }
        }

        fn register(&self, registry: &mut ToolRegistry) {
            registry.register(tools::report::CompilePdfReport);
        }
    }
}

fn official_packs() -> Vec<Box<dyn CapabilityPack>> {
    vec![
        Box::new(packs::ResearchPack),
        Box::new(packs::RehabilitationPack),
        Box::new(packs::PdfReportPack),
    ]
}

pub fn register_official(
    config: &CapabilityConfig,
    tools: &mut ToolRegistry,
) -> Vec<CapabilityManifest> {
    let mut registry = CapabilityRegistry::default();
    for pack in official_packs() {
        if config.enables(pack.manifest().id) {
            let manifest = pack.manifest();
            if registry.manifests.iter().any(|item| item.id == manifest.id) {
                continue;
            }
            pack.register(tools);
            registry.manifests.push(manifest);
        }
    }
    registry.manifests
}

pub fn official_statuses(config: &CapabilityConfig) -> Vec<CapabilityStatus> {
    official_packs()
        .into_iter()
        .map(|pack| {
            let manifest = pack.manifest();
            CapabilityStatus {
                enabled: config.enables(manifest.id),
                manifest,
            }
        })
        .collect()
}

pub fn official_manifests() -> Vec<CapabilityManifest> {
    let mut tools = ToolRegistry::new();
    tools.register_kernel();
    register_official(&CapabilityConfig::default(), &mut tools)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_activation_is_idempotent() {
        let mut tools = ToolRegistry::new();
        tools.register_kernel();
        let mut packs = CapabilityRegistry::default();
        packs.activate(packs::PdfReportPack, &mut tools);
        packs.activate(packs::PdfReportPack, &mut tools);

        assert_eq!(packs.manifests().len(), 1);
        assert!(tools
            .definitions()
            .iter()
            .any(|tool| tool.name == "compile_pdf_report"));
    }

    #[test]
    fn official_packs_have_unique_ids_and_tools() {
        let manifests = official_manifests();
        assert_eq!(manifests.len(), 3);
        for (index, manifest) in manifests.iter().enumerate() {
            assert!(!manifest.tool_names.is_empty());
            assert!(!manifest.ui_slots.is_empty());
            assert!(!manifests[..index]
                .iter()
                .any(|other| other.id == manifest.id));
        }
    }

    #[test]
    fn kernel_registry_has_no_domain_tools() {
        let mut tools = ToolRegistry::new();
        tools.register_kernel();
        let names = tools
            .definitions()
            .into_iter()
            .map(|tool| tool.name)
            .collect::<Vec<_>>();

        assert!(names.contains(&"read_file".to_string()));
        assert!(names.contains(&"execute_command".to_string()));
        for domain_tool in [
            "search_pubmed",
            "rehab_data",
            "create_research_plan",
            "compile_pdf_report",
        ] {
            assert!(!names.contains(&domain_tool.to_string()));
        }
    }

    #[test]
    fn config_can_disable_a_pack_without_changing_the_kernel() {
        let config = CapabilityConfig {
            enabled: Some(vec!["galen.pdf-report".into()]),
        };
        let mut tools = ToolRegistry::new();
        tools.register_kernel();
        let manifests = register_official(&config, &mut tools);
        let names = tools
            .definitions()
            .into_iter()
            .map(|tool| tool.name)
            .collect::<std::collections::HashSet<_>>();

        assert_eq!(manifests.len(), 1);
        assert!(names.contains("compile_pdf_report"));
        assert!(!names.contains("search_pubmed"));
        assert!(!names.contains("rehab_data"));
    }
}
