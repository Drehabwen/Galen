//! Recoverable BM25 index for the active task's evidence ledger.
//!
//! The JSONL evidence ledger remains the source of truth. Tantivy stores only
//! a derived index beside the active task and rebuilds it whenever the ledger
//! fingerprint changes, so deleting or corrupting the index never loses data.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{
    Field, IndexRecordOption, Schema, TextFieldIndexing, TextOptions, Value as _, STORED, STRING,
};
use tantivy::tokenizer::{LowerCaser, RemoveLongFilter, TextAnalyzer};
use tantivy::{doc, Index, TantivyDocument};

use crate::evidence::Evidence;

const INDEX_SCHEMA_VERSION: u32 = 1;
const TOKENIZER_NAME: &str = "galen_jieba";
const INDEX_DIR_NAME: &str = "evidence-index";
const INDEX_STATE_NAME: &str = "evidence-index-state.json";
const INDEX_WRITER_HEAP_BYTES: usize = 15_000_000;

static INDEX_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct IndexState {
    schema_version: u32,
    fingerprint: u64,
    documents: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvidenceSearchHit {
    pub score: f32,
    pub evidence: Evidence,
}

struct SearchFields {
    id: Field,
    node_title: Field,
    claim: Field,
    detail: Field,
}

/// Search the active task's durable evidence ledger with BM25.
///
/// The first call after evidence changes refreshes the derived on-disk index.
/// Subsequent calls open the committed index without rebuilding it.
pub fn search_evidence(
    workspace_root: &Path,
    query: &str,
    limit: usize,
) -> Result<Vec<EvidenceSearchHit>, String> {
    let query = query.trim();
    if query.is_empty() {
        return Err("证据检索词不能为空".to_string());
    }
    let limit = limit.clamp(1, 20);
    let _guard = INDEX_LOCK
        .lock()
        .map_err(|_| "证据索引锁已损坏，请重启 Galen 后重试".to_string())?;

    let evidence = crate::evidence::load_evidence(workspace_root)?;
    if evidence.is_empty() {
        return Ok(Vec::new());
    }
    let task_dir = crate::research_task::active_task_dir(workspace_root)?
        .ok_or("当前工作区没有活动研究任务")?;
    let (schema, fields) = build_schema();
    let index = open_index(&task_dir, schema)?;
    register_tokenizer(&index);
    refresh_if_stale(&index, &task_dir, &fields, &evidence)?;

    let reader = index
        .reader()
        .map_err(|error| format!("打开证据索引读取器失败: {error}"))?;
    let searcher = reader.searcher();
    let parser =
        QueryParser::for_index(&index, vec![fields.node_title, fields.claim, fields.detail]);
    let (parsed_query, _warnings) = parser.parse_query_lenient(query);
    let top_docs = searcher
        .search(&parsed_query, &TopDocs::with_limit(limit).order_by_score())
        .map_err(|error| format!("执行证据检索失败: {error}"))?;
    let by_id: HashMap<&str, &Evidence> = evidence
        .iter()
        .map(|item| (item.id.as_str(), item))
        .collect();

    let mut hits = Vec::with_capacity(top_docs.len());
    for (score, address) in top_docs {
        let document: TantivyDocument = searcher
            .doc(address)
            .map_err(|error| format!("读取证据检索结果失败: {error}"))?;
        let Some(id) = document
            .get_first(fields.id)
            .and_then(|value| value.as_str().map(str::to_owned))
        else {
            continue;
        };
        if let Some(item) = by_id.get(id.as_str()) {
            hits.push(EvidenceSearchHit {
                score,
                evidence: (*item).clone(),
            });
        }
    }
    Ok(hits)
}

fn build_schema() -> (Schema, SearchFields) {
    let mut builder = Schema::builder();
    let id = builder.add_text_field("id", STRING | STORED);
    let indexed_text = TextOptions::default()
        .set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer(TOKENIZER_NAME)
                .set_index_option(IndexRecordOption::WithFreqsAndPositions),
        )
        .set_stored();
    let node_title = builder.add_text_field("node_title", indexed_text.clone());
    let claim = builder.add_text_field("claim", indexed_text.clone());
    let detail = builder.add_text_field("detail", indexed_text);
    (
        builder.build(),
        SearchFields {
            id,
            node_title,
            claim,
            detail,
        },
    )
}

fn open_index(task_dir: &Path, schema: Schema) -> Result<Index, String> {
    let index_dir = task_dir.join(INDEX_DIR_NAME);
    std::fs::create_dir_all(&index_dir)
        .map_err(|error| format!("创建证据索引目录失败: {error}"))?;
    Index::open_or_create(
        tantivy::directory::MmapDirectory::open(&index_dir)
            .map_err(|error| format!("打开证据索引目录失败: {error}"))?,
        schema,
    )
    .map_err(|error| format!("打开或创建证据索引失败: {error}"))
}

fn register_tokenizer(index: &Index) {
    let analyzer = TextAnalyzer::builder(tantivy_jieba::JiebaTokenizer::new())
        .filter(RemoveLongFilter::limit(80))
        .filter(LowerCaser)
        .build();
    index.tokenizers().register(TOKENIZER_NAME, analyzer);
}

fn refresh_if_stale(
    index: &Index,
    task_dir: &Path,
    fields: &SearchFields,
    evidence: &[Evidence],
) -> Result<(), String> {
    let desired = IndexState {
        schema_version: INDEX_SCHEMA_VERSION,
        fingerprint: evidence_fingerprint(evidence)?,
        documents: evidence.len(),
    };
    let state_path = task_dir.join(INDEX_STATE_NAME);
    if read_state(&state_path).as_ref() == Some(&desired) {
        return Ok(());
    }

    let mut writer = index
        .writer(INDEX_WRITER_HEAP_BYTES)
        .map_err(|error| format!("创建证据索引写入器失败: {error}"))?;
    writer
        .delete_all_documents()
        .map_err(|error| format!("清理旧证据索引失败: {error}"))?;
    for item in evidence {
        writer
            .add_document(doc!(
                fields.id => item.id.clone(),
                fields.node_title => item.node_title.clone(),
                fields.claim => item.claim.clone(),
                fields.detail => item.detail.clone().unwrap_or_default(),
            ))
            .map_err(|error| format!("写入证据索引失败: {error}"))?;
    }
    writer
        .commit()
        .map_err(|error| format!("提交证据索引失败: {error}"))?;
    let state_json = serde_json::to_string_pretty(&desired)
        .map_err(|error| format!("序列化证据索引状态失败: {error}"))?;
    std::fs::write(&state_path, state_json)
        .map_err(|error| format!("保存证据索引状态失败: {error}"))
}

fn evidence_fingerprint(evidence: &[Evidence]) -> Result<u64, String> {
    let encoded =
        serde_json::to_vec(evidence).map_err(|error| format!("计算证据索引指纹失败: {error}"))?;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    INDEX_SCHEMA_VERSION.hash(&mut hasher);
    encoded.hash(&mut hasher);
    Ok(hasher.finish())
}

fn read_state(path: &Path) -> Option<IndexState> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_workspace(tag: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "galen-evidence-search-{}-{tag}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        crate::research_task::create_task(
            &path,
            "脊柱侧弯研究".to_string(),
            "验证中文康复证据检索".to_string(),
            Vec::new(),
        )
        .unwrap();
        path
    }

    fn evidence(id: &str, title: &str, claim: &str) -> Evidence {
        Evidence {
            id: id.to_string(),
            node_id: id.to_string(),
            node_title: title.to_string(),
            source: "research".to_string(),
            claim: claim.to_string(),
            detail: None,
            confidence: "medium".to_string(),
            created_at: "2026-08-25".to_string(),
        }
    }

    #[test]
    fn searches_chinese_rehabilitation_evidence() {
        let root = temp_workspace("chinese");
        crate::evidence::append_evidence_file(
            &root,
            evidence(
                "e1",
                "脊柱侧弯运动干预",
                "施罗斯运动可能改善青少年特发性脊柱侧弯的躯干旋转角",
            ),
        )
        .unwrap();
        crate::evidence::append_evidence_file(
            &root,
            evidence("e2", "脑卒中步态训练", "机器人训练可能改善步行速度"),
        )
        .unwrap();

        let hits = search_evidence(&root, "脊柱侧弯 施罗斯", 5).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].evidence.id, "e1");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn refreshes_after_ledger_changes_without_duplicate_hits() {
        let root = temp_workspace("refresh");
        crate::evidence::append_evidence_file(
            &root,
            evidence("e1", "初始证据", "核心稳定训练改善平衡"),
        )
        .unwrap();
        assert_eq!(search_evidence(&root, "核心稳定", 5).unwrap().len(), 1);

        crate::evidence::append_evidence_file(
            &root,
            evidence("e2", "补充证据", "核心稳定训练改善生活质量"),
        )
        .unwrap();
        let hits = search_evidence(&root, "核心稳定", 5).unwrap();
        assert_eq!(hits.len(), 2);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn empty_ledger_returns_no_hits_without_creating_index() {
        let root = temp_workspace("empty");
        assert!(search_evidence(&root, "康复", 5).unwrap().is_empty());
        let task_dir = crate::research_task::active_task_dir(&root)
            .unwrap()
            .unwrap();
        assert!(!task_dir.join(INDEX_DIR_NAME).exists());
        let _ = std::fs::remove_dir_all(root);
    }
}
