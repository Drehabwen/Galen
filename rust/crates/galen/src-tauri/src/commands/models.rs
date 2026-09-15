use super::{lock_mutex, AppState};
use api::{InputContentBlock, InputMessage, MessageRequest};
use tauri::State;

#[derive(serde::Serialize)]
pub struct ModelStatus {
    pub name: String,
    pub model_id: String,
    pub description: Option<String>,
    pub api_key_present: bool,
    pub api_key_masked: Option<String>,
    pub base_url: Option<String>,
    pub max_tokens: Option<u32>,
    pub is_default: bool,
}

fn mask_key(key: &str) -> String {
    if key.len() <= 8 {
        "••••".to_string()
    } else {
        format!("{}…{}", &key[..4], &key[key.len() - 4..])
    }
}

/// Report configured models and whether each has an API key (masked only).
#[tauri::command]
pub fn get_model_status(state: State<AppState>) -> Result<Vec<ModelStatus>, String> {
    let backend = lock_mutex(&state.backend)?;
    let default_alias = backend.router.default_alias().to_string();
    let mut statuses: Vec<ModelStatus> = backend
        .router
        .all_models()
        .iter()
        .map(|(alias, entry)| {
            let masked = entry
                .api_key
                .as_deref()
                .filter(|key| !key.is_empty())
                .map(mask_key);
            ModelStatus {
                name: alias.clone(),
                model_id: entry.model_id.clone(),
                description: entry.description.clone(),
                api_key_present: masked.is_some(),
                api_key_masked: masked,
                base_url: entry.base_url.clone(),
                max_tokens: entry.max_tokens,
                is_default: *alias == default_alias,
            }
        })
        .collect();
    statuses.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(statuses)
}

/// Test the default model connection with a minimal ping request.
#[tauri::command]
pub async fn test_model_connection(state: State<'_, AppState>) -> Result<String, String> {
    let (_alias, model_id, client) = {
        let backend = lock_mutex(&state.backend)?;
        let alias = backend.router.default_alias().to_string();
        let model_id = backend.router.resolve_model_id(&alias);
        let router = backend.router.clone();
        let client = super::backend::make_client(&alias, &router)?;
        (alias, model_id, client)
    };

    let request = MessageRequest {
        model: model_id.clone(),
        max_tokens: 8,
        messages: vec![InputMessage {
            role: "user".to_string(),
            content: vec![InputContentBlock::Text {
                text: "ping".to_string(),
            }],
        }],
        system: None,
        tools: None,
        tool_choice: None,
        stream: false,
        temperature: None,
        top_p: None,
        frequency_penalty: None,
        presence_penalty: None,
        stop: None,
        reasoning_effort: None,
        thinking: None,
    };

    match tokio::time::timeout(
        std::time::Duration::from_secs(30),
        client.send_message(&request),
    )
    .await
    {
        Ok(Ok(_)) => Ok(format!("连接成功：{model_id} 响应正常")),
        Ok(Err(error)) => Err(format!("连接失败：{error}")),
        Err(_) => Err("连接超时（30 秒），请检查网络或 API Key".to_string()),
    }
}
