//! Workspace-scoped user decision memory.
//!
//! This ledger records explicit constraints and corrections without another
//! model call. It is intentionally small: the raw conversation remains the
//! audit trail, while this file keeps the user-authored decisions that must
//! survive compaction and application restarts.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use serde::{Deserialize, Serialize};

static DECISION_STORE_LOCK: Mutex<()> = Mutex::new(());
const MAX_DECISION_CHARS: usize = 360;
const DEFAULT_CONTEXT_DECISIONS: usize = 12;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DecisionRecord {
    #[serde(default)]
    pub id: String,
    pub timestamp_ms: u64,
    pub statement: String,
    #[serde(default = "default_topic")]
    pub topic: String,
    #[serde(default)]
    pub status: DecisionStatus,
    #[serde(default)]
    pub superseded_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum DecisionStatus {
    #[default]
    Active,
    Superseded,
    Dismissed,
}

pub fn capture_user_decisions(
    workspace: &Path,
    user_text: &str,
    timestamp_ms: u64,
) -> Result<Vec<DecisionRecord>, String> {
    let candidates = extract_decision_statements(user_text);
    if candidates.is_empty() {
        return Ok(Vec::new());
    }

    let _guard = lock_store()?;
    let mut existing = load_all_unlocked(workspace)?;
    let mut appended = Vec::new();
    for (index, statement) in candidates.into_iter().enumerate() {
        if existing
            .iter()
            .chain(appended.iter())
            .any(|item| item.statement == statement && item.status == DecisionStatus::Active)
        {
            continue;
        }
        let topic = classify_topic(&statement);
        let id = format!("decision-{timestamp_ms}-{index}");
        for previous in existing.iter_mut().chain(appended.iter_mut()) {
            if previous.status == DecisionStatus::Active && previous.topic == topic {
                previous.status = DecisionStatus::Superseded;
                previous.superseded_by = Some(id.clone());
            }
        }
        let record = DecisionRecord {
            id,
            timestamp_ms,
            statement,
            topic,
            status: DecisionStatus::Active,
            superseded_by: None,
        };
        appended.push(record);
    }
    existing.extend(appended.iter().cloned());
    persist_all_unlocked(workspace, &existing)?;
    Ok(appended)
}

pub fn load_recent_decisions(
    workspace: &Path,
    limit: Option<usize>,
) -> Result<Vec<DecisionRecord>, String> {
    let _guard = lock_store()?;
    let records = load_all_unlocked(workspace)?;
    let limit = limit.unwrap_or(DEFAULT_CONTEXT_DECISIONS);
    Ok(records
        .into_iter()
        .rev()
        .take(limit)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect())
}

pub fn render_decision_context(workspace: &Path) -> Result<Option<String>, String> {
    let records = load_recent_decisions(workspace, None)?
        .into_iter()
        .filter(|record| record.status == DecisionStatus::Active)
        .collect::<Vec<_>>();
    if records.is_empty() {
        return Ok(None);
    }
    let statements = records
        .iter()
        .map(|record| format!("- {}", record.statement))
        .collect::<Vec<_>>()
        .join("\n");
    Ok(Some(format!(
        "[用户决策账本]\n以下条目来自用户此前的明确约束和修订，按时间由旧到新排列。发生冲突时，以更新的明确修订为准；不得自行恢复被替代的旧值。\n{statements}"
    )))
}

pub fn dismiss_decision(workspace: &Path, id: &str) -> Result<(), String> {
    let _guard = lock_store()?;
    let mut records = load_all_unlocked(workspace)?;
    let record = records
        .iter_mut()
        .find(|record| record.id == id)
        .ok_or_else(|| "未找到该共识记录".to_string())?;
    record.status = DecisionStatus::Dismissed;
    persist_all_unlocked(workspace, &records)
}

pub fn revise_decision(
    workspace: &Path,
    id: &str,
    statement: &str,
    timestamp_ms: u64,
) -> Result<DecisionRecord, String> {
    let statement = statement.trim();
    if statement.is_empty() {
        return Err("共识内容不能为空".to_string());
    }
    let _guard = lock_store()?;
    let mut records = load_all_unlocked(workspace)?;
    let position = records
        .iter()
        .position(|record| record.id == id)
        .ok_or_else(|| "未找到该共识记录".to_string())?;
    let topic = records[position].topic.clone();
    let new_id = format!("decision-{timestamp_ms}-revision");
    records[position].status = DecisionStatus::Superseded;
    records[position].superseded_by = Some(new_id.clone());
    let revised = DecisionRecord {
        id: new_id,
        timestamp_ms,
        statement: truncate_chars(statement, MAX_DECISION_CHARS),
        topic,
        status: DecisionStatus::Active,
        superseded_by: None,
    };
    records.push(revised.clone());
    persist_all_unlocked(workspace, &records)?;
    Ok(revised)
}

fn extract_decision_statements(text: &str) -> Vec<String> {
    text.split_inclusive(|character| {
        matches!(
            character,
            '\n' | '。' | '！' | '!' | ';' | '；' | '?' | '？'
        )
    })
    .map(str::trim)
    .filter(|segment| !segment.is_empty())
    .filter(|segment| is_decision_candidate(segment))
    .map(|segment| {
        segment.trim_end_matches(|character| {
            matches!(character, '\n' | '。' | '！' | '!' | ';' | '；')
        })
    })
    .map(|segment| truncate_chars(segment, MAX_DECISION_CHARS))
    .take(8)
    .collect()
}

fn is_decision_candidate(text: &str) -> bool {
    let operational_instruction = [
        "不要创建文件",
        "不要调用工具",
        "不调用工具",
        "无需工具",
        "最终聊天",
        "聊天回答",
        "只回复",
        "请明确列出",
        "请列出",
        "回忆上一轮",
        "承接上一轮",
        "不要让我重复",
        "文件必须包含",
        "为了测试",
    ]
    .iter()
    .any(|cue| text.contains(cue));
    if operational_instruction {
        return false;
    }
    let correction_or_rule = [
        "决定",
        "确定",
        "定为",
        "改为",
        "改成",
        "调整为",
        "更正",
        "保持",
        "不变",
        "必须",
        "不要",
        "默认",
        "优先",
        "采用",
        "核心约束",
        "请记住",
    ]
    .iter()
    .any(|cue| text.contains(cue));
    let research_constraint = [
        "协议号",
        "样本量",
        "主要结局",
        "次要结局",
        "随访",
        "纳入标准",
        "排除标准",
        "研究对象",
        "干预组",
        "对照组",
    ]
    .iter()
    .any(|cue| text.contains(cue));
    if text.contains(['?', '？']) && !correction_or_rule {
        return false;
    }
    correction_or_rule || research_constraint
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    let mut chars = text.chars();
    let head = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{head}…")
    } else {
        head
    }
}

fn default_topic() -> String {
    "general".to_string()
}

fn classify_topic(statement: &str) -> String {
    const TOPICS: &[(&str, &[&str])] = &[
        ("protocol", &["协议号"]),
        ("sample_size", &["样本量"]),
        ("primary_outcome", &["主要结局"]),
        ("secondary_outcome", &["次要结局"]),
        ("follow_up", &["随访"]),
        ("population", &["研究对象", "纳入标准", "排除标准"]),
        ("intervention", &["干预组"]),
        ("control", &["对照组"]),
    ];
    TOPICS
        .iter()
        .find(|(_, cues)| cues.iter().any(|cue| statement.contains(cue)))
        .map(|(topic, _)| (*topic).to_string())
        .unwrap_or_else(|| format!("general:{}", stable_topic_key(statement)))
}

fn stable_topic_key(statement: &str) -> String {
    statement
        .chars()
        .filter(|character| !character.is_whitespace())
        .take(24)
        .collect()
}

fn load_all_unlocked(workspace: &Path) -> Result<Vec<DecisionRecord>, String> {
    let path = decisions_path(workspace);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content =
        std::fs::read_to_string(&path).map_err(|error| format!("读取决策账本失败: {error}"))?;
    let mut records = content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str::<DecisionRecord>(line)
                .map_err(|error| format!("解析决策账本失败: {error}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    for (index, record) in records.iter_mut().enumerate() {
        if record.id.is_empty() {
            record.id = format!("legacy-{}-{index}", record.timestamp_ms);
        }
        if record.topic == "general" {
            record.topic = classify_topic(&record.statement);
        }
    }
    Ok(records)
}

fn persist_all_unlocked(workspace: &Path, records: &[DecisionRecord]) -> Result<(), String> {
    let path = decisions_path(workspace);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("创建对话记忆目录失败: {error}"))?;
    }
    let mut content = records
        .iter()
        .map(|record| serde_json::to_string(record).map_err(|error| error.to_string()))
        .collect::<Result<Vec<_>, _>>()?
        .join("\n");
    if !content.is_empty() {
        content.push('\n');
    }
    std::fs::write(path, content).map_err(|error| format!("写入决策账本失败: {error}"))
}

fn decisions_path(workspace: &Path) -> PathBuf {
    workspace
        .join(".galen")
        .join("conversation-decisions.jsonl")
}

fn lock_store() -> Result<MutexGuard<'static, ()>, String> {
    DECISION_STORE_LOCK
        .lock()
        .map_err(|_| "决策账本锁已损坏".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "galen-conversation-memory-{}-{label}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn captures_constraints_and_corrections_but_not_questions() {
        let root = workspace("capture");
        let records = capture_user_decisions(
            &root,
            "协议号 GALEN-73，样本量定为 48。随访是多少？把随访从 12 周改成 16 周。",
            1,
        )
        .unwrap();
        assert_eq!(records.len(), 2);
        assert!(records[0].statement.contains("样本量"));
        assert!(records[1].statement.contains("改成 16 周"));
        assert!(records
            .iter()
            .all(|record| !record.statement.contains("多少")));
    }

    #[test]
    fn renders_recent_decisions_in_chronological_order_without_duplicates() {
        let root = workspace("render");
        capture_user_decisions(&root, "样本量定为 48。", 1).unwrap();
        capture_user_decisions(&root, "样本量定为 48。随访改成 16 周。", 2).unwrap();
        let records = load_recent_decisions(&root, None).unwrap();
        assert_eq!(records.len(), 2);
        let context = render_decision_context(&root).unwrap().unwrap();
        assert!(context.find("样本量").unwrap() < context.find("随访").unwrap());
        assert!(context.contains("更新的明确修订为准"));
    }

    #[test]
    fn newer_decision_supersedes_the_same_topic() {
        let root = workspace("supersede");
        capture_user_decisions(&root, "随访定为 12 周。", 1).unwrap();
        capture_user_decisions(&root, "随访改成 16 周。", 2).unwrap();
        let records = load_recent_decisions(&root, None).unwrap();
        assert_eq!(records[0].status, DecisionStatus::Superseded);
        assert_eq!(records[1].status, DecisionStatus::Active);
        let context = render_decision_context(&root).unwrap().unwrap();
        assert!(!context.contains("12 周"));
        assert!(context.contains("16 周"));
    }

    #[test]
    fn revised_and_dismissed_decisions_change_active_context() {
        let root = workspace("manual-revision");
        let original = capture_user_decisions(&root, "样本量定为 48。", 1)
            .unwrap()
            .remove(0);
        let revised = revise_decision(&root, &original.id, "样本量定为 56", 2).unwrap();
        let context = render_decision_context(&root).unwrap().unwrap();
        assert!(context.contains("56"));
        assert!(!context.contains("48"));
        dismiss_decision(&root, &revised.id).unwrap();
        assert!(render_decision_context(&root).unwrap().is_none());
    }

    #[test]
    fn excludes_operational_chat_instructions_from_research_consensus() {
        let root = workspace("operational-noise");
        let records = capture_user_decisions(
            &root,
            "不要创建文件。承接上一轮，不要让我重复项目约束。文件必须包含协议号、样本量和主要结局。现在把随访从 12 周改成 16 周，但其他核心约束不变。请明确列出新旧随访。",
            1,
        )
        .unwrap();
        assert_eq!(records.len(), 1);
        assert!(records[0].statement.contains("改成 16 周"));
    }
}
