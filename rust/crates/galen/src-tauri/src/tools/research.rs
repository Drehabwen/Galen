use super::{GalenTool, ToolContext};
use crate::backend::ChatEvent;
use crate::research_task::ResearchNode;
use api::ToolDefinition;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;

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
