use super::workspace_path::resolve_workspace_path;
use super::{GalenTool, ToolContext};
use api::ToolDefinition;
use async_trait::async_trait;
use serde_json::{json, Value};

/// Search the active research task's durable local evidence ledger.
pub struct SearchEvidence;

#[async_trait]
impl GalenTool for SearchEvidence {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "search_evidence".into(),
            description: Some(
                "Search the active Galen research task's local evidence ledger with Chinese-aware BM25. Use this before repeating an external literature search; results contain durable claims, details, confidence and node provenance."
                    .into(),
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Chinese or English evidence query"
                    },
                    "limit": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 20,
                        "default": 5
                    }
                },
                "required": ["query"]
            }),
        }
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String, String> {
        let query = input
            .get("query")
            .and_then(Value::as_str)
            .ok_or("Missing 'query'")?;
        let limit = input
            .get("limit")
            .and_then(Value::as_u64)
            .unwrap_or(5)
            .clamp(1, 20) as usize;
        let root = resolve_workspace_path(&ctx.workspace_root, "")?;
        let results = crate::evidence_search::search_evidence(&root, query, limit)?;
        serde_json::to_string_pretty(&json!({
            "engine": "tantivy-bm25",
            "tokenizer": "jieba-search-mode",
            "query": query,
            "result_count": results.len(),
            "results": results,
        }))
        .map_err(|error| format!("序列化证据检索结果失败: {error}"))
    }
}
