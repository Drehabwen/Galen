//! Workspace-scoped durable chat sessions for the Galen UI.
//!
//! The shared runtime owns the append-only JSONL format and compaction events;
//! this module only maps Galen's simple chat messages onto that durable model.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use api::{InputContentBlock, InputMessage};
use runtime::{
    compact_session, CompactionConfig, ContentBlock, ConversationMessage, MessageRole, Session,
    TurnRecord, TurnStatus,
};
use serde::Serialize;

static CHAT_SESSION_STORE_LOCK: Mutex<()> = Mutex::new(());

const PRESERVE_RECENT_MESSAGES: usize = 8;
const COMPACT_AT_ESTIMATED_TOKENS: usize = 72_000;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ChatSessionMessage {
    pub role: String,
    pub content: String,
    pub timestamp: u64,
}

pub fn prepare_model_history(
    workspace: &Path,
    tag: Option<&str>,
    model: &str,
    fallback: Vec<InputMessage>,
) -> Result<Vec<InputMessage>, String> {
    let _guard = lock_store()?;
    let mut session = load_or_create(workspace, tag, model)?;
    if session.messages.is_empty() && !fallback.is_empty() {
        for message in &fallback {
            if let Some(message) = api_to_runtime(message) {
                session.push_message(message).map_err(session_error)?;
            }
        }
    }

    let compacted = compact_session(
        &session,
        CompactionConfig {
            preserve_recent_messages: PRESERVE_RECENT_MESSAGES,
            max_estimated_tokens: COMPACT_AT_ESTIMATED_TOKENS,
        },
    );
    if let Some(error) = compacted.persist_error {
        return Err(format!("保存会话压缩记录失败: {error}"));
    }
    Ok(compacted
        .compacted_session
        .messages
        .iter()
        .filter_map(runtime_to_api)
        .collect())
}

pub fn append_exchange(
    workspace: &Path,
    tag: Option<&str>,
    model: &str,
    user_text: &str,
    assistant_text: &str,
    started_at_ms: u64,
    metrics: &crate::backend::ChatRunSummary,
) -> Result<(), String> {
    let _guard = lock_store()?;
    let mut session = load_or_create(workspace, tag, model)?;

    let incomplete_user_already_present = session.messages.last().is_some_and(|message| {
        message.role == MessageRole::User && message_text(message).trim() == user_text.trim()
    });
    if !incomplete_user_already_present {
        session
            .push_prompt_entry(user_text.to_string())
            .map_err(session_error)?;
        session
            .push_user_text(user_text.to_string())
            .map_err(session_error)?;
    }
    session
        .push_message(ConversationMessage::assistant(vec![ContentBlock::Text {
            text: assistant_text.to_string(),
        }]))
        .map_err(session_error)?;
    let turn_index = session.turn_history.len().saturating_add(1) as u32;
    session
        .record_turn(TurnRecord {
            turn_index,
            started_at_ms,
            completed_at_ms: now_millis_u64(),
            status: TurnStatus::Completed,
            user_input: user_text.to_string(),
            iterations: metrics.iterations as usize,
            tool_call_count: metrics.tool_call_count,
            usage_input_tokens: metrics
                .input_tokens
                .saturating_add(metrics.cache_creation_input_tokens)
                .saturating_add(metrics.cache_read_input_tokens),
            usage_output_tokens: metrics.output_tokens,
            error: None,
        })
        .map_err(session_error)
}

pub fn load_messages(
    workspace: &Path,
    tag: Option<&str>,
) -> Result<Vec<ChatSessionMessage>, String> {
    let _guard = lock_store()?;
    let path = session_path(workspace, tag)?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let session = Session::load_from_path(&path).map_err(session_error)?;
    verify_workspace(&session, workspace)?;
    let base = session.created_at_ms;
    Ok(session
        .messages
        .iter()
        .enumerate()
        .filter_map(|(index, message)| {
            let role = match message.role {
                MessageRole::User => "user",
                MessageRole::Assistant => "assistant",
                MessageRole::System => "assistant",
                MessageRole::Tool => return None,
            };
            let content = message_text(message);
            if content.trim().is_empty() {
                return None;
            }
            Some(ChatSessionMessage {
                role: role.to_string(),
                content,
                timestamp: base.saturating_add(index as u64),
            })
        })
        .collect())
}

/// Archive instead of deleting so Ctrl+L remains recoverable.
pub fn archive_session(workspace: &Path, tag: Option<&str>) -> Result<(), String> {
    let _guard = lock_store()?;
    let source = session_path(workspace, tag)?;
    if !source.exists() {
        return Ok(());
    }
    let archive_dir = sessions_dir(workspace).join("archive");
    std::fs::create_dir_all(&archive_dir)
        .map_err(|error| format!("创建会话归档目录失败: {error}"))?;
    let stem = source
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("session");
    let target = archive_dir.join(format!("{stem}-{}.jsonl", now_millis()));
    std::fs::rename(&source, &target).map_err(|error| format!("归档会话失败: {error}"))
}

fn load_or_create(workspace: &Path, tag: Option<&str>, model: &str) -> Result<Session, String> {
    let path = session_path(workspace, tag)?;
    std::fs::create_dir_all(sessions_dir(workspace))
        .map_err(|error| format!("创建会话目录失败: {error}"))?;
    if path.exists() {
        let session = Session::load_from_path(&path).map_err(session_error)?;
        verify_workspace(&session, workspace)?;
        return Ok(session);
    }

    let mut session = Session::new()
        .with_workspace_root(workspace.to_path_buf())
        .with_persistence_path(path);
    session.model = Some(model.to_string());
    session.ensure_persisted().map_err(session_error)?;
    Ok(session)
}

fn verify_workspace(session: &Session, workspace: &Path) -> Result<(), String> {
    if let Some(bound) = session.workspace_root() {
        if bound != workspace {
            return Err(format!(
                "会话工作区不匹配：日志绑定到 {}，当前为 {}",
                bound.display(),
                workspace.display()
            ));
        }
    }
    Ok(())
}

fn api_to_runtime(message: &InputMessage) -> Option<ConversationMessage> {
    let role = match message.role.as_str() {
        "user" => MessageRole::User,
        "assistant" => MessageRole::Assistant,
        _ => return None,
    };
    let text = message
        .content
        .iter()
        .filter_map(|block| match block {
            InputContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    if text.trim().is_empty() {
        return None;
    }
    Some(ConversationMessage {
        role,
        blocks: vec![ContentBlock::Text { text }],
        usage: None,
    })
}

fn runtime_to_api(message: &ConversationMessage) -> Option<InputMessage> {
    let role = match message.role {
        MessageRole::System => "system",
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
        MessageRole::Tool => return None,
    };
    let text = message_text(message);
    if text.trim().is_empty() {
        return None;
    }
    Some(InputMessage {
        role: role.to_string(),
        content: vec![InputContentBlock::Text { text }],
    })
}

fn message_text(message: &ConversationMessage) -> String {
    message
        .blocks
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            ContentBlock::ToolUse { .. } | ContentBlock::ToolResult { .. } => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn sessions_dir(workspace: &Path) -> PathBuf {
    workspace.join(".galen").join("sessions")
}

fn session_path(workspace: &Path, tag: Option<&str>) -> Result<PathBuf, String> {
    let key = validate_tag(tag)?;
    Ok(sessions_dir(workspace).join(format!("{key}.jsonl")))
}

fn validate_tag(tag: Option<&str>) -> Result<&str, String> {
    let tag = tag.filter(|value| !value.is_empty()).unwrap_or("main");
    if tag.len() > 100
        || !tag
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Err("无效会话标识：只允许字母、数字、连字符和下划线".to_string());
    }
    Ok(tag)
}

fn lock_store() -> Result<MutexGuard<'static, ()>, String> {
    CHAT_SESSION_STORE_LOCK
        .lock()
        .map_err(|_| "会话存储锁已损坏".to_string())
}

fn session_error(error: impl std::fmt::Display) -> String {
    format!("会话存储失败: {error}")
}

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn now_millis_u64() -> u64 {
    u64::try_from(now_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_workspace(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "galen-chat-session-{}-{label}-{}",
            std::process::id(),
            now_millis()
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn persists_and_restores_an_exchange() {
        let workspace = temp_workspace("roundtrip");
        let metrics = crate::backend::ChatRunSummary {
            iterations: 3,
            tool_call_count: 2,
            input_tokens: 100,
            output_tokens: 20,
            cache_creation_input_tokens: 5,
            cache_read_input_tokens: 50,
        };
        append_exchange(
            &workspace,
            None,
            "deepseek-v4-pro",
            "问题",
            "回答",
            1,
            &metrics,
        )
        .unwrap();
        let messages = load_messages(&workspace, None).unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].content, "问题");
        assert_eq!(messages[1].content, "回答");
        let session = Session::load_from_path(session_path(&workspace, None).unwrap()).unwrap();
        assert_eq!(session.prompt_history.len(), 1);
        assert_eq!(session.turn_history.len(), 1);
        assert_eq!(session.turn_history[0].iterations, 3);
        assert_eq!(session.turn_history[0].tool_call_count, 2);
        assert_eq!(session.turn_history[0].usage_input_tokens, 155);
        assert_eq!(session.turn_history[0].usage_output_tokens, 20);
    }

    #[test]
    fn migrates_frontend_history_only_into_an_empty_session() {
        let workspace = temp_workspace("migration");
        let fallback = vec![InputMessage {
            role: "user".to_string(),
            content: vec![InputContentBlock::Text {
                text: "旧问题".to_string(),
            }],
        }];
        let first = prepare_model_history(&workspace, Some("node-01"), "model", fallback).unwrap();
        let second =
            prepare_model_history(&workspace, Some("node-01"), "model", Vec::new()).unwrap();
        assert_eq!(first.len(), 1);
        assert_eq!(second.len(), 1);
    }

    #[test]
    fn rejects_path_traversal_tags() {
        let workspace = temp_workspace("traversal");
        assert!(load_messages(&workspace, Some("../escape")).is_err());
    }

    #[test]
    fn clear_archives_instead_of_deleting() {
        let workspace = temp_workspace("archive");
        append_exchange(
            &workspace,
            None,
            "model",
            "问题",
            "回答",
            1,
            &crate::backend::ChatRunSummary::default(),
        )
        .unwrap();
        archive_session(&workspace, None).unwrap();
        assert!(load_messages(&workspace, None).unwrap().is_empty());
        assert_eq!(
            std::fs::read_dir(sessions_dir(&workspace).join("archive"))
                .unwrap()
                .count(),
            1
        );
    }
}
