//! 结构化证据链：节点回流的产出落为 Evidence 对象，
//! 供上下文注入（L2 证据链层）与最终成文引用。

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::Path;

/// 一条结构化证据（节点回流产出）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Evidence {
    pub id: String,
    pub node_id: String,
    pub node_title: String,
    /// 来源类型：planning / research / data / analysis / writing ...
    pub source: String,
    /// 核心结论（≤200 字，用于上下文注入）
    pub claim: String,
    /// 完整摘要（可选，最终成文时引用）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// 置信度：high / medium / low
    pub confidence: String,
    pub created_at: String,
}

/// Append one evidence record to the active task's append-only JSONL ledger.
pub fn append_evidence_file(
    root: &Path,
    evidence: Evidence,
) -> Result<crate::research_task::ResearchTask, String> {
    migrate_legacy_evidence(root)?;
    let task_dir = crate::research_task::active_task_dir(root)?
        .ok_or("当前工作区没有活动研究任务，无法保存证据")?;
    let path = task_dir.join("evidence.jsonl");

    // Retrying a flow-back must not duplicate an already durable record.
    if load_evidence(root)?
        .iter()
        .any(|item| item.id == evidence.id)
    {
        return crate::research_task::attach_evidence_ids(root, &[evidence.id]);
    }

    std::fs::create_dir_all(&task_dir).map_err(|e| format!("创建证据目录失败: {e}"))?;
    let line = serde_json::to_string(&evidence).map_err(|e| format!("序列化证据失败: {e}"))?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| format!("打开证据账本失败: {e}"))?;
    writeln!(file, "{line}").map_err(|e| format!("写入证据账本失败: {e}"))?;
    file.sync_data()
        .map_err(|e| format!("同步证据账本失败: {e}"))?;
    crate::research_task::attach_evidence_ids(root, &[evidence.id])
}

/// Read the active task's complete evidence ledger.
pub fn load_evidence(root: &Path) -> Result<Vec<Evidence>, String> {
    migrate_legacy_evidence(root)?;
    let Some(task_dir) = crate::research_task::active_task_dir(root)? else {
        return Ok(Vec::new());
    };
    let path = task_dir.join("evidence.jsonl");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = std::fs::read_to_string(&path).map_err(|e| format!("读取证据账本失败: {e}"))?;
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .enumerate()
        .map(|(index, line)| {
            serde_json::from_str(line).map_err(|e| format!("证据账本第 {} 行无效: {e}", index + 1))
        })
        .collect()
}

/// 证据链摘要（每节点一行），供 L2 上下文注入。
/// 按时间取最近 `max` 条。
pub fn evidence_chain_summary(root: &Path, max: usize) -> String {
    let list = match load_evidence(root) {
        Ok(list) => list,
        Err(error) => return format!("\n\n## 证据链\n证据账本不可读：{error}"),
    };
    if list.is_empty() {
        return String::new();
    }
    let mut out = String::from("\n\n## 证据链（已回流节点产出）");
    for ev in list.iter().rev().take(max).rev() {
        let claim: String = ev.claim.chars().take(100).collect();
        out.push_str(&format!(
            "\n- [{source}] {title} → {claim}（{confidence}）",
            source = ev.source,
            title = ev.node_title,
            confidence = ev.confidence
        ));
    }
    out
}

/// Copy the previous root-level array into the active task ledger once. The
/// source is intentionally retained so migration is recoverable.
fn migrate_legacy_evidence(root: &Path) -> Result<(), String> {
    let Some(task_dir) = crate::research_task::active_task_dir(root)? else {
        return Ok(());
    };
    let target = task_dir.join("evidence.jsonl");
    if target.exists() {
        return Ok(());
    }
    let legacy = root.join("evidence.json");
    if !legacy.exists() {
        return Ok(());
    }
    let text =
        std::fs::read_to_string(&legacy).map_err(|e| format!("读取旧 evidence.json 失败: {e}"))?;
    let evidence: Vec<Evidence> =
        serde_json::from_str(&text).map_err(|e| format!("旧 evidence.json 无法迁移: {e}"))?;
    std::fs::create_dir_all(&task_dir).map_err(|e| format!("创建证据目录失败: {e}"))?;
    let pending = task_dir.join("evidence.jsonl.pending");
    let mut content = String::new();
    for item in &evidence {
        content
            .push_str(&serde_json::to_string(item).map_err(|e| format!("序列化旧证据失败: {e}"))?);
        content.push('\n');
    }
    std::fs::write(&pending, content).map_err(|e| format!("写入证据迁移文件失败: {e}"))?;
    std::fs::rename(&pending, &target).map_err(|e| format!("保存迁移后的证据账本失败: {e}"))?;
    let ids: Vec<String> = evidence.into_iter().map(|item| item.id).collect();
    crate::research_task::attach_evidence_ids(root, &ids).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("galen_ev_{}_{}", std::process::id(), tag));
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    fn create_task(dir: &Path) {
        crate::research_task::create_task(
            dir,
            "证据测试".to_string(),
            "验证任务级证据账本".to_string(),
            Vec::new(),
        )
        .unwrap();
    }

    fn sample(id: &str, title: &str) -> Evidence {
        Evidence {
            id: id.into(),
            node_id: id.into(),
            node_title: title.into(),
            source: "research".into(),
            claim: format!("{title} 的核心结论"),
            detail: Some("详细过程...".into()),
            confidence: "medium".into(),
            created_at: "2026-08-13".into(),
        }
    }

    #[test]
    fn appends_and_loads() {
        let dir = tmp_dir("append");
        create_task(&dir);
        append_evidence_file(&dir, sample("s01", "文献检索")).unwrap();
        append_evidence_file(&dir, sample("s02", "数据分析")).unwrap();
        let list = load_evidence(&dir).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[1].node_title, "数据分析");
        assert!(dir
            .join(".galen/tasks")
            .read_dir()
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path()
            .join("evidence.jsonl")
            .exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn summary_lists_recent_claims() {
        let dir = tmp_dir("summary");
        create_task(&dir);
        append_evidence_file(&dir, sample("s01", "文献检索")).unwrap();
        append_evidence_file(&dir, sample("s02", "数据分析")).unwrap();
        let s = evidence_chain_summary(&dir, 8);
        assert!(s.contains("证据链"));
        assert!(s.contains("数据分析 的核心结论"));
        assert!(s.contains("medium"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn empty_chain_yields_empty_summary() {
        let dir = tmp_dir("empty");
        create_task(&dir);
        assert_eq!(evidence_chain_summary(&dir, 8), "");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn migrates_legacy_evidence_without_deleting_source() {
        let dir = tmp_dir("legacy");
        create_task(&dir);
        let legacy = vec![sample("s01", "旧证据")];
        std::fs::write(
            dir.join("evidence.json"),
            serde_json::to_string_pretty(&legacy).unwrap(),
        )
        .unwrap();
        let loaded = load_evidence(&dir).unwrap();
        assert_eq!(loaded, legacy);
        assert!(dir.join("evidence.json").exists());
        let task = crate::research_task::load_active_task(&dir)
            .unwrap()
            .unwrap();
        assert_eq!(task.evidence_ids, vec!["s01"]);
        assert_eq!(task.revision, 2);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
