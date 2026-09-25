//! Authoritative defaults used only when the user has not supplied a model catalog.
//! Runtime model selection must continue to come from `models.toml`.

pub const DEFAULT_FAST_MODEL: &str = "deepseek-v4-flash";
pub const DEFAULT_ANALYSIS_MODEL: &str = "deepseek-v4-pro";
pub const DEFAULT_DEEPSEEK_BASE_URL: &str = "https://api.deepseek.com/v1";
pub const DEFAULT_OPENAI_BASE_URL: &str = "https://api.openai.com/v1";
pub const OPENAI_COMPAT_PROVIDER: &str = "openai_compat";

fn model_entry(api_key: &str, model_id: &str, description: &str) -> toml::Value {
    let mut entry = toml::Table::new();
    entry.insert(
        "provider".into(),
        toml::Value::String(OPENAI_COMPAT_PROVIDER.into()),
    );
    entry.insert("api_key".into(), toml::Value::String(api_key.into()));
    entry.insert("model_id".into(), toml::Value::String(model_id.into()));
    entry.insert(
        "base_url".into(),
        toml::Value::String(DEFAULT_DEEPSEEK_BASE_URL.into()),
    );
    entry.insert("description".into(), toml::Value::String(description.into()));
    entry.insert("max_tokens".into(), toml::Value::Integer(32_768));
    toml::Value::Table(entry)
}

pub fn imported_deepseek_model(api_key: &str) -> toml::Value {
    model_entry(api_key, DEFAULT_ANALYSIS_MODEL, "DeepSeek V4 Pro（导入）")
}

pub fn default_models_toml(api_key: &str, default_model: Option<&str>) -> Result<String, String> {
    let default_model = default_model.unwrap_or(DEFAULT_FAST_MODEL);
    if ![DEFAULT_FAST_MODEL, DEFAULT_ANALYSIS_MODEL].contains(&default_model) {
        return Err(format!("默认模型不在推荐模型目录中: {default_model}"));
    }

    let mut router = toml::Table::new();
    router.insert("default".into(), toml::Value::String(default_model.into()));
    router.insert("fast".into(), toml::Value::String(DEFAULT_FAST_MODEL.into()));
    router.insert(
        "analysis".into(),
        toml::Value::String(DEFAULT_ANALYSIS_MODEL.into()),
    );

    let mut models = toml::Table::new();
    models.insert(
        DEFAULT_ANALYSIS_MODEL.into(),
        model_entry(api_key, DEFAULT_ANALYSIS_MODEL, "DeepSeek V4 Pro（深度研究）"),
    );
    models.insert(
        DEFAULT_FAST_MODEL.into(),
        model_entry(api_key, DEFAULT_FAST_MODEL, "DeepSeek V4 Flash（默认，快速）"),
    );

    let mut root = toml::Table::new();
    root.insert("router".into(), toml::Value::Table(router));
    root.insert("models".into(), toml::Value::Table(models));
    toml::to_string_pretty(&toml::Value::Table(root))
        .map_err(|error| format!("序列化默认模型配置失败: {error}"))
}

pub fn missing_model_message() -> String {
    format!(
        "未配置可用模型：请在 ~/.galen/models.toml 中配置模型，或在欢迎向导中保存 API Key。Galen 当前推荐 model_id = \"{}\"、provider = \"{}\"、base_url = \"{}\"。",
        DEFAULT_FAST_MODEL, OPENAI_COMPAT_PROVIDER, DEFAULT_DEEPSEEK_BASE_URL
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_toml_escapes_secrets_and_is_loadable() {
        let content = default_models_toml("sk-quote-\"-line\nnext", None).unwrap();
        let value = content.parse::<toml::Value>().unwrap();
        assert_eq!(
            value["models"][DEFAULT_FAST_MODEL]["api_key"].as_str(),
            Some("sk-quote-\"-line\nnext")
        );
    }

    #[test]
    fn generated_toml_rejects_unknown_default_alias() {
        assert!(default_models_toml("sk-test", Some("unknown-model")).is_err());
    }
}
