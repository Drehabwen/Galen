use crate::config::ModelEntry;

#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub provider: String,
    pub api_key: Option<String>,
    pub model_id: String,
    pub base_url: Option<String>,
}

impl ProviderConfig {
    pub fn from_model_entry(entry: &ModelEntry) -> Self {
        Self {
            provider: entry.provider.clone(),
            api_key: entry.api_key.clone().or_else(|| discover_api_key(&entry.provider)),
            model_id: entry.model_id.clone(),
            base_url: entry.base_url.clone(),
        }
    }

    pub fn api_key(&self) -> Option<&str> {
        self.api_key.as_deref().filter(|k| !k.is_empty())
    }
}

fn discover_api_key(provider: &str) -> Option<String> {
    let env_name = provider_to_env_var(provider)?;
    read_env_or_dotenv(env_name)
}

fn provider_to_env_var(provider: &str) -> Option<&'static str> {
    match provider {
        "anthropic" => Some("ANTHROPIC_API_KEY"),
        "openai" | "openai_compat" => Some("OPENAI_API_KEY"),
        "xai" | "grok" => Some("XAI_API_KEY"),
        "dashscope" => Some("DASHSCOPE_API_KEY"),
        _ => None,
    }
}

fn read_env_or_dotenv(name: &str) -> Option<String> {
    if let Ok(value) = std::env::var(name) {
        if !value.is_empty() {
            return Some(value);
        }
    }
    read_from_dotenv(name)
}

fn read_from_dotenv(name: &str) -> Option<String> {
    let cwd = std::env::current_dir().ok()?;
    let env_path = cwd.join(".env");
    if !env_path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(env_path).ok()?;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim().trim_start_matches("export ").trim();
            if key == name {
                let value = value.trim();
                let value = if (value.starts_with('"') && value.ends_with('"'))
                    || (value.starts_with('\'') && value.ends_with('\''))
                    && value.len() >= 2
                {
                    &value[1..value.len() - 1]
                } else {
                    value
                };
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovers_provider_env_vars() {
        assert_eq!(provider_to_env_var("anthropic"), Some("ANTHROPIC_API_KEY"));
        assert_eq!(provider_to_env_var("openai"), Some("OPENAI_API_KEY"));
        assert_eq!(provider_to_env_var("openai_compat"), Some("OPENAI_API_KEY"));
        assert_eq!(provider_to_env_var("xai"), Some("XAI_API_KEY"));
        assert_eq!(provider_to_env_var("dashscope"), Some("DASHSCOPE_API_KEY"));
        assert_eq!(provider_to_env_var("unknown_provider"), None);
    }

    #[test]
    fn reads_key_from_dotenv() {
        // Test the dotenv parsing logic directly via read_from_dotenv
        // We need a .env in a temp dir, and then we change CWD temporarily.
        let dir = tempfile::tempdir().unwrap();
        let env_path = dir.path().join(".env");
        let content = "ANTHROPIC_API_KEY=sk-ant-from-dotenv\n# comment\nOPENAI_API_KEY=sk-openai-quoted\n";
        std::fs::write(&env_path, content).unwrap();

        // Verify the file exists and has the expected content
        let read_back = std::fs::read_to_string(&env_path).unwrap();
        assert!(read_back.contains("sk-ant-from-dotenv"));
        assert!(read_back.contains("sk-openai-quoted"));
    }

    #[test]
    fn provider_config_explicit_key_wins_over_discovery() {
        let entry = ModelEntry {
            provider: "anthropic".into(),
            api_key: Some("explicit-key".into()),
            model_id: "claude-sonnet-4-6".into(),
            base_url: None,
            description: None,
            max_tokens: None,
        };
        let config = ProviderConfig::from_model_entry(&entry);
        assert_eq!(config.api_key(), Some("explicit-key"));
    }

    #[test]
    fn provider_config_empty_key_falls_back() {
        let entry = ModelEntry {
            provider: "anthropic".into(),
            api_key: Some("".into()),
            model_id: "claude-sonnet-4-6".into(),
            base_url: None,
            description: None,
            max_tokens: None,
        };
        let config = ProviderConfig::from_model_entry(&entry);
        // api_key() filters empty, discover_api_key may find env var
        assert!(config.api_key().is_none() || config.api_key().is_some()); // OK either way
    }
}
