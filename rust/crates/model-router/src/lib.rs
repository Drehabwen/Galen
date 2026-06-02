pub mod config;
pub mod discovery;

use config::{load_models_toml, ConfigError, ModelEntry, ModelsToml};
use discovery::ProviderConfig;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskKind {
    Chat,
    QuickLookup,
    DeepAnalysis,
    CodeGen,
}

impl TaskKind {
    pub fn from_intent(text: &str) -> Self {
        let lower = text.to_lowercase();
        if lower.contains("查") || lower.contains("什么是") || lower.contains("解释") || lower.contains("定义") {
            return Self::QuickLookup;
        }
        if lower.contains("综述")
            || lower.contains("分析")
            || lower.contains("精读")
            || lower.contains("方法学")
            || lower.contains("全文")
        {
            return Self::DeepAnalysis;
        }
        if lower.contains("代码") || lower.contains("编程") || lower.contains("bug") || lower.contains("编译") {
            return Self::CodeGen;
        }
        Self::Chat
    }
}

#[derive(Debug, Clone)]
pub struct ModelRouter {
    config: ModelsToml,
}

impl ModelRouter {
    pub fn load() -> Result<Self, ConfigError> {
        let config = load_models_toml()?;
        if !config.models.is_empty() && !config.models.contains_key(&config.router.default) {
            return Err(ConfigError::MissingDefaultModel);
        }
        Ok(Self { config })
    }

    pub fn load_from(path: &std::path::Path) -> Result<Self, ConfigError> {
        let config = config::load_models_toml_from(path)?;
        if !config.models.is_empty() && !config.models.contains_key(&config.router.default) {
            return Err(ConfigError::MissingDefaultModel);
        }
        Ok(Self { config })
    }

    pub fn route(&self, task: TaskKind) -> Option<&ModelEntry> {
        let alias = match task {
            TaskKind::QuickLookup => self
                .config
                .router
                .fast
                .as_deref()
                .unwrap_or(&self.config.router.default),
            TaskKind::DeepAnalysis => self
                .config
                .router
                .analysis
                .as_deref()
                .unwrap_or(&self.config.router.default),
            _ => &self.config.router.default,
        };
        self.config.models.get(alias)
    }

    pub fn get_model(&self, alias: &str) -> Option<&ModelEntry> {
        self.config.models.get(alias)
    }

    pub fn default_model(&self) -> Option<&ModelEntry> {
        self.config.models.get(&self.config.router.default)
    }

    pub fn all_models(&self) -> &HashMap<String, ModelEntry> {
        &self.config.models
    }

    pub fn resolve_model_id(&self, alias: &str) -> String {
        self.get_model(alias)
            .map(|m| m.model_id.clone())
            .unwrap_or_else(|| alias.to_string())
    }

    pub fn to_provider_config(&self, alias: &str) -> Option<ProviderConfig> {
        self.get_model(alias)
            .map(ProviderConfig::from_model_entry)
    }

    pub fn has_models(&self) -> bool {
        !self.config.models.is_empty()
    }
}

impl Default for ModelRouter {
    fn default() -> Self {
        Self {
            config: ModelsToml::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn test_router() -> ModelRouter {
        let toml_str = r#"
[router]
default = "opus"
fast = "haiku"
analysis = "sonnet"

[models.opus]
provider = "anthropic"
api_key = "sk-ant-opus"
model_id = "claude-opus-4-6"

[models.haiku]
provider = "anthropic"
api_key = "sk-ant-haiku"
model_id = "claude-haiku-4-5"

[models.sonnet]
provider = "anthropic"
api_key = "sk-ant-sonnet"
model_id = "claude-sonnet-4-6"
description = "Deep analysis model"
"#;
        let mut tmp = tempfile::Builder::new()
            .suffix(".toml")
            .tempfile()
            .unwrap();
        write!(tmp, "{toml_str}").unwrap();
        let path = tmp.path().to_path_buf();
        let router = ModelRouter::load_from(&path).unwrap();
        let _ = tmp.close();
        router
    }

    #[test]
    fn routes_chat_to_default() {
        let router = test_router();
        let model = router.route(TaskKind::Chat).unwrap();
        assert_eq!(model.model_id, "claude-opus-4-6");
    }

    #[test]
    fn routes_quick_lookup_to_fast() {
        let router = test_router();
        let model = router.route(TaskKind::QuickLookup).unwrap();
        assert_eq!(model.model_id, "claude-haiku-4-5");
    }

    #[test]
    fn routes_deep_analysis_to_analysis() {
        let router = test_router();
        let model = router.route(TaskKind::DeepAnalysis).unwrap();
        assert_eq!(model.model_id, "claude-sonnet-4-6");
    }

    #[test]
    fn routes_code_gen_to_default() {
        let router = test_router();
        let model = router.route(TaskKind::CodeGen).unwrap();
        assert_eq!(model.model_id, "claude-opus-4-6");
    }

    #[test]
    fn quick_lookup_falls_back_to_default_when_fast_missing() {
        let toml_str = r#"
[router]
default = "main"

[models.main]
provider = "anthropic"
model_id = "claude-sonnet-4-6"
"#;
        let mut tmp = tempfile::Builder::new()
            .suffix(".toml")
            .tempfile()
            .unwrap();
        write!(tmp, "{toml_str}").unwrap();
        let router = ModelRouter::load_from(tmp.path()).unwrap();
        let _ = tmp.close();

        let model = router.route(TaskKind::QuickLookup).unwrap();
        assert_eq!(model.model_id, "claude-sonnet-4-6");
    }

    #[test]
    fn resolves_model_id_from_alias() {
        let router = test_router();
        assert_eq!(router.resolve_model_id("opus"), "claude-opus-4-6");
        assert_eq!(router.resolve_model_id("unknown"), "unknown");
    }

    #[test]
    fn builds_provider_config() {
        let router = test_router();
        let config = router.to_provider_config("opus").unwrap();
        assert_eq!(config.provider, "anthropic");
        assert_eq!(config.model_id, "claude-opus-4-6");
    }

    #[test]
    fn task_kind_from_text_quick_lookup() {
        assert_eq!(
            TaskKind::from_intent("什么是单核苷酸多态性"),
            TaskKind::QuickLookup
        );
        assert_eq!(
            TaskKind::from_intent("查一下二甲双胍的作用"),
            TaskKind::QuickLookup
        );
        assert_eq!(
            TaskKind::from_intent("解释一下什么是SNP"),
            TaskKind::QuickLookup
        );
    }

    #[test]
    fn task_kind_from_text_deep_analysis() {
        assert_eq!(
            TaskKind::from_intent("帮我写一篇关于阿尔茨海默病的综述"),
            TaskKind::DeepAnalysis
        );
        assert_eq!(
            TaskKind::from_intent("精读这篇文献的方法学部分"),
            TaskKind::DeepAnalysis
        );
        assert_eq!(
            TaskKind::from_intent("帮我分析一下这篇论文的统计方法"),
            TaskKind::DeepAnalysis
        );
    }

    #[test]
    fn task_kind_from_text_chat_default() {
        assert_eq!(TaskKind::from_intent("你好"), TaskKind::Chat);
        assert_eq!(TaskKind::from_intent("今天天气不错"), TaskKind::Chat);
    }

    #[test]
    fn empty_models_config_is_ok() {
        let toml_str = "";
        let mut tmp = tempfile::Builder::new()
            .suffix(".toml")
            .tempfile()
            .unwrap();
        write!(tmp, "{toml_str}").unwrap();
        let router = ModelRouter::load_from(tmp.path()).unwrap();
        let _ = tmp.close();

        assert!(!router.has_models());
        assert!(router.route(TaskKind::Chat).is_none());
    }
}
