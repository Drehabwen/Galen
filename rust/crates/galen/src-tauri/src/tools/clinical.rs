use api::ToolDefinition;
use async_trait::async_trait;
use serde_json::{json, Value};

use super::{GalenTool, ToolContext};

pub struct AnalyzeClinicalCase;

#[async_trait]
impl GalenTool for AnalyzeClinicalCase {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "analyze_clinical_case".into(),
            description: Some(
                "Analyze a clinical case description and generate a structured reasoning report \
                 covering differential diagnosis, risk factors, and recommended next steps.".into(),
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "case_text": {"type": "string", "description": "Clinical case description"},
                    "age": {"type": "integer", "description": "Patient age (optional)"},
                    "sex": {"type": "string", "description": "Patient sex (optional)"},
                    "context": {"type": "string", "description": "Additional context (optional)"}
                },
                "required": ["case_text"]
            }),
        }
    }

    async fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<String, String> {
        let case_text = input["case_text"].as_str().ok_or("Missing 'case_text'")?;
        if case_text.trim().is_empty() {
            return Err("Please provide a case description.".into());
        }
        let age = input["age"].as_u64().map(|v| v as u8);
        let sex = input["sex"].as_str().map(|s| s.to_string());
        let context = input["context"].as_str().map(|s| s.to_string());
        medical_core::clinical::run(
            medical_core::clinical::ClinicalCaseInput { case_text: case_text.into(), age, sex, context },
            "markdown",
        )
    }
}
