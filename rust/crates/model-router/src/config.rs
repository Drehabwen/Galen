use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ModelsToml {
    #[serde(default)]
    pub router: RouterSection,
    #[serde(default)]
    pub models: HashMap<String, ModelEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RouterSection {
    #[serde(default = "default_role")]
    pub default: String,
    #[serde(default)]
    pub fast: Option<String>,
    #[serde(default)]
    pub analysis: Option<String>,
}

impl Default for RouterSection {
    fn default() -> Self {
        Self {
            default: default_role(),
            fast: None,
            analysis: None,
        }
    }
}

fn default_role() -> String {
    "default".to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModelEntry {
    pub provider: String,
    #[serde(default)]
    pub api_key: Option<String>,
    pub model_id: String,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
}

#[derive(Debug)]
pub enum ConfigError {
    Io(std::io::Error),
    Parse(toml::de::Error),
    MissingDefaultModel,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "failed to read models.toml: {e}"),
            Self::Parse(e) => write!(f, "invalid models.toml: {e}"),
            Self::MissingDefaultModel => {
                write!(f, "models.toml: [router].default model not found in [models]")
            }
        }
    }
}

impl std::error::Error for ConfigError {}

impl From<std::io::Error> for ConfigError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<toml::de::Error> for ConfigError {
    fn from(e: toml::de::Error) -> Self {
        Self::Parse(e)
    }
}

fn config_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Ok(cwd) = std::env::current_dir() {
        paths.push(cwd.join("models.toml"));
    }

    if let Some(home) = dirs_fallback() {
        paths.push(home.join(".claw").join("models.toml"));
        paths.push(home.join("models.toml"));
    }

    paths
}

fn dirs_fallback() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

pub fn load_models_toml() -> Result<ModelsToml, ConfigError> {
    for path in config_paths() {
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            let config: ModelsToml = toml::from_str(&content)?;
            if !config.models.contains_key(&config.router.default) {
                return Err(ConfigError::MissingDefaultModel);
            }
            return Ok(config);
        }
    }
    Ok(ModelsToml::default())
}

pub fn load_models_toml_from(path: &std::path::Path) -> Result<ModelsToml, ConfigError> {
    let content = std::fs::read_to_string(path)?;
    let config: ModelsToml = toml::from_str(&content)?;
    if !config.models.is_empty() && !config.models.contains_key(&config.router.default) {
        return Err(ConfigError::MissingDefaultModel);
    }
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn parses_minimal_config() {
        let toml_str = r#"
[router]
default = "main"

[models.main]
provider = "anthropic"
model_id = "claude-sonnet-4-6"
api_key = "sk-ant-test"
"#;
        let config: ModelsToml = toml::from_str(toml_str).unwrap();
        assert_eq!(config.router.default, "main");
        assert_eq!(config.models["main"].provider, "anthropic");
        assert_eq!(config.models["main"].model_id, "claude-sonnet-4-6");
        assert_eq!(config.models["main"].api_key.as_deref(), Some("sk-ant-test"));
    }

    #[test]
    fn parses_multi_model_config() {
        let toml_str = r#"
[router]
default = "opus"
fast = "haiku"
analysis = "sonnet"

[models.opus]
provider = "anthropic"
api_key = "sk-ant-xxx"
model_id = "claude-opus-4-6"

[models.haiku]
provider = "anthropic"
api_key = "sk-ant-xxx"
model_id = "claude-haiku-4-5"

[models.local]
provider = "openai_compat"
base_url = "http://localhost:11434/v1"
model_id = "qwen2.5:7b"
"#;
        let config: ModelsToml = toml::from_str(toml_str).unwrap();
        assert_eq!(config.router.default, "opus");
        assert_eq!(config.router.fast.as_deref(), Some("haiku"));
        assert_eq!(config.router.analysis.as_deref(), Some("sonnet"));
        assert_eq!(config.models.len(), 3);
        assert!(config.models["local"].api_key.is_none());
        assert_eq!(
            config.models["local"].base_url.as_deref(),
            Some("http://localhost:11434/v1")
        );
    }

    #[test]
    fn defaults_router_when_absent() {
        let toml_str = r#"
[models.default]
provider = "anthropic"
model_id = "claude-sonnet-4-6"
"#;
        let config: ModelsToml = toml::from_str(toml_str).unwrap();
        assert_eq!(config.router.default, "default");
        assert!(config.router.fast.is_none());
    }

    #[test]
    fn rejects_missing_default_model() {
        let toml_str = r#"
[router]
default = "nonexistent"

[models.other]
provider = "anthropic"
model_id = "claude-sonnet-4-6"
"#;
        let err = load_models_toml_from_str(toml_str).unwrap_err();
        assert!(matches!(err, ConfigError::MissingDefaultModel));
    }

    #[test]
    fn parses_optional_fields() {
        let toml_str = r#"
[router]
default = "main"

[models.main]
provider = "openai_compat"
model_id = "gpt-4o"
description = "OpenAI GPT-4o via OpenRouter"
max_tokens = 4096
"#;
        let config: ModelsToml = toml::from_str(toml_str).unwrap();
        let model = &config.models["main"];
        assert_eq!(model.description.as_deref(), Some("OpenAI GPT-4o via OpenRouter"));
        assert_eq!(model.max_tokens, Some(4096));
    }

    fn load_models_toml_from_str(s: &str) -> Result<ModelsToml, ConfigError> {
        let mut tmp = tempfile::Builder::new()
            .suffix(".toml")
            .tempfile()
            .unwrap();
        write!(tmp, "{s}").unwrap();
        let path = tmp.path();
        let result = load_models_toml_from(path);
        let _ = tmp.close();
        result
    }
}
