//! 结构化证据链：节点回流的产出落为 Evidence 对象，
//! 供上下文注入（L2 证据链层）与最终成文引用。

use serde::{Deserialize, Serialize};
use std::path::Path;

/// 一条结构化证据（节点回流产出）
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// 追加一条证据到 `<workspace>/evidence.json`
pub fn append_evidence_file(root: &Path, evidence: Evidence) -> Result<(), String> {
    let path = root.join("evidence.json");
    let mut list: Vec<Evidence> = if path.exists() {
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    list.push(evidence);
    let json = serde_json::to_string_pretty(&list).map_err(|e| format!("序列化失败: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("写入 evidence.json 失败: {e}"))
}

/// 读取全部证据
pub fn load_evidence(root: &Path) -> Vec<Evidence> {
    let path = root.join("evidence.json");
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

/// 证据链摘要（每节点一行），供 L2 上下文注入。
/// 按时间取最近 `max` 条。
pub fn evidence_chain_summary(root: &Path, max: usize) -> String {
    let list = load_evidence(root);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("galen_ev_{}_{}", std::process::id(), tag));
        let _ = std::fs::create_dir_all(&dir);
        dir
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
        append_evidence_file(&dir, sample("s01", "文献检索")).unwrap();
        append_evidence_file(&dir, sample("s02", "数据分析")).unwrap();
        let list = load_evidence(&dir);
        assert_eq!(list.len(), 2);
        assert_eq!(list[1].node_title, "数据分析");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn summary_lists_recent_claims() {
        let dir = tmp_dir("summary");
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
        assert_eq!(evidence_chain_summary(&dir, 8), "");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
