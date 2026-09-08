//! Task-scoped PRISMA screening state.
//!
//! Identification is calculated from the durable search-run ledger. The later
//! screening stages remain researcher-entered because Galen must not invent a
//! duplicate count, exclusion decision, or included-study count.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReviewFlow {
    pub task_id: Option<String>,
    /// Raw records returned by durable successful/partial search runs.
    pub identified: u32,
    pub duplicates_removed: Option<u32>,
    pub screened: Option<u32>,
    pub excluded: Option<u32>,
    pub full_text_assessed: Option<u32>,
    pub included: Option<u32>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewFlowInput {
    pub duplicates_removed: Option<u32>,
    pub screened: Option<u32>,
    pub excluded: Option<u32>,
    pub full_text_assessed: Option<u32>,
    pub included: Option<u32>,
}

pub fn load_review_flow(workspace: &Path) -> Result<ReviewFlow, String> {
    let Some(task) = crate::research_task::load_active_task(workspace)? else {
        return Ok(ReviewFlow {
            task_id: None,
            identified: 0,
            duplicates_removed: None,
            screened: None,
            excluded: None,
            full_text_assessed: None,
            included: None,
            updated_at: None,
        });
    };
    let identified = identified_records(workspace, &task.task_id)?;
    let path = review_flow_path(workspace, &task.task_id);
    let mut flow = if path.exists() {
        let text = std::fs::read_to_string(&path)
            .map_err(|error| format!("读取 PRISMA 筛选记录失败: {error}"))?;
        serde_json::from_str::<ReviewFlow>(&text)
            .map_err(|error| format!("PRISMA 筛选记录格式无效: {error}"))?
    } else {
        ReviewFlow {
            task_id: Some(task.task_id.clone()),
            identified,
            duplicates_removed: None,
            screened: None,
            excluded: None,
            full_text_assessed: None,
            included: None,
            updated_at: None,
        }
    };
    flow.task_id = Some(task.task_id);
    flow.identified = identified;
    validate(&flow)?;
    Ok(flow)
}

pub fn save_review_flow(workspace: &Path, input: ReviewFlowInput) -> Result<ReviewFlow, String> {
    let task = crate::research_task::load_active_task(workspace)?
        .ok_or("当前没有活动研究任务，无法保存 PRISMA 筛选状态")?;
    let identified = identified_records(workspace, &task.task_id)?;
    let flow = ReviewFlow {
        task_id: Some(task.task_id.clone()),
        identified,
        duplicates_removed: input.duplicates_removed,
        screened: input.screened,
        excluded: input.excluded,
        full_text_assessed: input.full_text_assessed,
        included: input.included,
        updated_at: Some(now_timestamp()),
    };
    validate(&flow)?;
    let path = review_flow_path(workspace, &task.task_id);
    let parent = path.parent().ok_or("PRISMA 筛选记录路径无效")?;
    std::fs::create_dir_all(parent).map_err(|error| format!("创建 PRISMA 目录失败: {error}"))?;
    let pending = path.with_extension("json.pending");
    let text = serde_json::to_string_pretty(&flow)
        .map_err(|error| format!("序列化 PRISMA 筛选记录失败: {error}"))?;
    std::fs::write(&pending, text).map_err(|error| format!("写入 PRISMA 筛选记录失败: {error}"))?;
    if path.exists() {
        std::fs::remove_file(&path)
            .map_err(|error| format!("更新 PRISMA 筛选记录失败: {error}"))?;
    }
    std::fs::rename(&pending, &path)
        .map_err(|error| format!("保存 PRISMA 筛选记录失败: {error}"))?;
    Ok(flow)
}

fn identified_records(workspace: &Path, task_id: &str) -> Result<u32, String> {
    Ok(crate::search_run::load_search_runs(workspace, task_id)?
        .iter()
        .filter(|run| {
            matches!(
                run.status,
                crate::search_run::SearchRunStatus::Succeeded
                    | crate::search_run::SearchRunStatus::Partial
            )
        })
        .filter_map(|run| run.result_count)
        .fold(0_u32, |sum, value| sum.saturating_add(value as u32)))
}

fn review_flow_path(workspace: &Path, task_id: &str) -> PathBuf {
    workspace
        .join(".galen")
        .join("tasks")
        .join(task_id)
        .join("review-flow.json")
}

fn validate(flow: &ReviewFlow) -> Result<(), String> {
    let post_dedupe = flow
        .identified
        .saturating_sub(flow.duplicates_removed.unwrap_or(0));
    if flow.duplicates_removed.unwrap_or(0) > flow.identified {
        return Err("去重数不能大于实际检出记录数。".into());
    }
    if flow.screened.is_some_and(|value| value > post_dedupe) {
        return Err("初筛数不能大于去重后的记录数。".into());
    }
    if let (Some(excluded), Some(screened)) = (flow.excluded, flow.screened) {
        if excluded > screened {
            return Err("初筛排除数不能大于初筛记录数。".into());
        }
    }
    if let (Some(full_text), Some(screened), Some(excluded)) =
        (flow.full_text_assessed, flow.screened, flow.excluded)
    {
        if full_text > screened.saturating_sub(excluded) {
            return Err("全文评估数不能大于初筛后保留的记录数。".into());
        }
    }
    if let (Some(included), Some(full_text)) = (flow.included, flow.full_text_assessed) {
        if included > full_text {
            return Err("纳入研究数不能大于全文评估数。".into());
        }
    }
    Ok(())
}

fn now_timestamp() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_secs().to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_conserving_screening_counts() {
        let flow = ReviewFlow {
            task_id: Some("task-1".into()),
            identified: 10,
            duplicates_removed: Some(2),
            screened: Some(8),
            excluded: Some(3),
            full_text_assessed: Some(6),
            included: Some(7),
            updated_at: None,
        };
        assert!(validate(&flow).is_err());
    }

    #[test]
    fn accepts_a_complete_prisma_path() {
        let flow = ReviewFlow {
            task_id: Some("task-1".into()),
            identified: 10,
            duplicates_removed: Some(2),
            screened: Some(8),
            excluded: Some(3),
            full_text_assessed: Some(5),
            included: Some(4),
            updated_at: None,
        };
        assert!(validate(&flow).is_ok());
    }
}
