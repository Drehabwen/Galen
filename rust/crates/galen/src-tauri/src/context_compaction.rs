use std::path::PathBuf;
use std::sync::Mutex;

use api::{InputContentBlock, InputMessage, MessageRequest, OutputContentBlock, ProviderClient};

/// 把被折叠的中间消息压缩为结构化摘要（额外一次轻量 LLM 调用）。
/// 只保留 user/assistant 的文本，跳过工具调用细节；失败时返回空串，
/// 由调用方回退为普通占位符。
pub(crate) async fn summarize_middle(
    client: &ProviderClient,
    model_id: &str,
    middle: &[InputMessage],
) -> Result<String, String> {
    let mut text_messages: Vec<InputMessage> = middle
        .iter()
        .filter(|m| m.role == "user" || m.role == "assistant")
        .map(|m| InputMessage {
            role: m.role.clone(),
            content: m
                .content
                .iter()
                .filter_map(|b| match b {
                    InputContentBlock::Text { text } => {
                        Some(InputContentBlock::Text { text: text.clone() })
                    }
                    _ => None,
                })
                .collect(),
        })
        .filter(|m| !m.content.is_empty())
        .collect();
    if text_messages.is_empty() {
        return Ok(String::new());
    }

    let mut messages = Vec::with_capacity(text_messages.len() + 1);
    messages.push(InputMessage {
        role: "user".to_string(),
        content: vec![InputContentBlock::Text {
            text: "请将以下对话压缩为结构化摘要：\n1. 对话中若包含以【已压缩摘要】开头的文本，必须完整保留其全部条目（那是更早轮次的摘要，一旦丢失将无法恢复）。\n2. 保留：研究目标、已完成的动作与结果、关键结论、未决问题。\n3. 出现过的文件路径、PMID、明确要求保留的数据条目逐项保留。\n用简洁条目输出，新增内容不要复述过程。"
                .to_string(),
        }],
    });
    messages.append(&mut text_messages);

    let request = MessageRequest {
        model: model_id.to_string(),
        max_tokens: 600,
        messages,
        system: None,
        tools: None,
        tool_choice: None,
        stream: false,
        temperature: Some(0.2),
        top_p: None,
        frequency_penalty: None,
        presence_penalty: None,
        stop: None,
        reasoning_effort: None,
        thinking: None,
    };

    // Compaction is a fallback, not user-visible work. Never let a slow
    // summarizer add another minute to time-to-first-result; the caller has a
    // deterministic placeholder when this short attempt fails.
    let resp = tokio::time::timeout(
        std::time::Duration::from_secs(12),
        client.send_message(&request),
    )
    .await
    .map_err(|_| "摘要生成超时".to_string())?
    .map_err(|e| format!("摘要生成失败: {e}"))?;

    let text = resp
        .content
        .iter()
        .filter_map(|b| match b {
            OutputContentBlock::Text { text } => Some(text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    Ok(text)
}

/// 把折叠摘要追加到工作区 `.galen/context-archive.md`，让被折叠的历史可追溯。
/// 写盘失败静默（压缩本身不因存档失败而失败）。
pub(crate) fn archive_compaction(workspace_root: &Mutex<Option<PathBuf>>, summary: &str) {
    if summary.trim().is_empty() {
        return;
    }
    let root = match workspace_root.lock().ok().and_then(|g| g.clone()) {
        Some(r) => r,
        None => return,
    };
    let dir = root.join(".galen");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    use std::io::Write;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let entry = format!("## 上下文压缩存档 (unix {now})\n{summary}\n\n");
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("context-archive.md"))
        .and_then(|mut f| f.write_all(entry.as_bytes()));
}
