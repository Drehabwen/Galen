use super::{GalenTool, ToolContext};
use crate::backend::ChatEvent;
use crate::research_task::ResearchNode;
use api::ToolDefinition;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;

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
}

const PUBMED_PATHS: &[&str] = &[""];
const CROSSREF_PATHS: &[&str] = &[
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

pub struct CreateResearchPlan;

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

#[cfg(test)]
mod search_catalog_tests {
    use super::*;

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
}
