use api::{
    ContentBlockDelta, InputContentBlock, InputMessage, MessageRequest, ProviderClient,
    StreamEvent as ApiStreamEvent,
};
use model_router::ModelRouter;
use tokio::sync::mpsc;

const KNOWN_MODELS: &[(&str, &str)] = &[
    ("opus", "claude-opus-4-6"),
    ("sonnet", "claude-sonnet-4-6"),
    ("haiku", "claude-haiku-4-5-20251001"),
    ("gpt-4o", "gpt-4o"),
    ("gpt-4.1", "gpt-4.1"),
];

#[derive(Debug, Clone)]
pub struct ModelConfig {
    pub name: String,
    pub model_id: String,
}

pub struct ChatBackend {
    pub router: ModelRouter,
}

impl ChatBackend {
    pub fn new() -> Self {
        let router = ModelRouter::load().unwrap_or_else(|e| {
            eprintln!("Failed to load models.toml: {e}, using defaults");
            ModelRouter::default()
        });
        Self { router }
    }

    pub fn default_model(&self) -> Option<ModelConfig> {
        if let Some(m) = self.router.default_model() {
            Some(ModelConfig {
                name: self.router.resolve_model_id(&m.model_id),
                model_id: m.model_id.clone(),
            })
        } else {
            KNOWN_MODELS.first().map(|(name, id)| ModelConfig {
                name: (*name).to_string(),
                model_id: (*id).to_string(),
            })
        }
    }

    pub fn all_models(&self) -> Vec<ModelConfig> {
        let configured = self.router.all_models();
        if configured.is_empty() {
            KNOWN_MODELS
                .iter()
                .map(|(name, id)| ModelConfig {
                    name: (*name).to_string(),
                    model_id: (*id).to_string(),
                })
                .collect()
        } else {
            configured
                .iter()
                .map(|(alias, entry)| ModelConfig {
                    name: alias.clone(),
                    model_id: entry.model_id.clone(),
                })
                .collect()
        }
    }

    pub fn resolve_model(&self, alias: &str) -> String {
        let resolved = self.router.resolve_model_id(alias);
        if resolved == alias {
            KNOWN_MODELS
                .iter()
                .find(|(name, _)| *name == alias)
                .map(|(_, id)| (*id).to_string())
                .unwrap_or_else(|| resolved)
        } else {
            resolved
        }
    }

    pub fn spawn_chat(
        model_id: String,
        user_message: String,
        conversation_history: Vec<InputMessage>,
        tx: mpsc::UnboundedSender<StreamEvent>,
    ) {
        tokio::spawn(async move {
            let result =
                Self::run_chat(&model_id, &user_message, conversation_history, tx.clone()).await;
            if let Err(e) = result {
                let _ = tx.send(StreamEvent::Error(format!("{e}")));
            }
        });
    }

    async fn run_chat(
        model_id: &str,
        user_message: &str,
        mut history: Vec<InputMessage>,
        tx: mpsc::UnboundedSender<StreamEvent>,
    ) -> Result<(), Box<dyn std::error::Error + Send>> {
        let client = ProviderClient::from_model(model_id)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send>)?;

        history.push(InputMessage {
            role: "user".to_string(),
            content: vec![InputContentBlock::Text {
                text: user_message.to_string(),
            }],
        });

        let request = MessageRequest {
            model: model_id.to_string(),
            messages: history,
            max_tokens: 4096,
            system: None,
            tools: None,
            tool_choice: None,
            stream: true,
            ..Default::default()
        };

        let mut stream = client
            .stream_message(&request)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send>)?;

        let mut full_text = String::new();
        loop {
            match stream.next_event().await {
                Ok(Some(ApiStreamEvent::ContentBlockDelta(event))) => {
                    if let ContentBlockDelta::TextDelta { text } = event.delta {
                        full_text.push_str(&text);
                        let _ = tx.send(StreamEvent::Delta(text));
                    }
                }
                Ok(Some(ApiStreamEvent::MessageStop(_))) => break,
                Ok(None) => break,
                Err(e) => {
                    let _ = tx.send(StreamEvent::Error(format!("stream error: {e}")));
                    break;
                }
                _ => {}
            }
        }

        let _ = tx.send(StreamEvent::Done(full_text));
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum StreamEvent {
    Delta(String),
    Done(String),
    Error(String),
}
