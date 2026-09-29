//! Rehab research data access tool.
//!
//! Reads the rehab research SQLite database (`blood.db`) with read-only
//! connections and exposes curated queries to the agent. Every result carries
//! a source header so the evidence chain stays traceable.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use api::ToolDefinition;
use async_trait::async_trait;
use rusqlite::{params_from_iter, types::Value, Connection, OpenFlags};
use serde_json::{json, Value as Json};

use super::{GalenTool, ToolContext};

const MAX_ROWS: usize = 200;

/// Resolve the rehab database path: `GALEN_REHAB_DB` env var, then
/// `~/.galen/rehab.toml` (`db_path = "..."`), then `<workspace>/blood.db`.
fn resolve_db_path(ctx: &ToolContext) -> Option<PathBuf> {
    if let Ok(p) = std::env::var("GALEN_REHAB_DB") {
        if !p.trim().is_empty() {
            return Some(PathBuf::from(p));
        }
    }
    if let Some(home) = dirs::home_dir() {
        let cfg = home.join(".galen").join("rehab.toml");
        if cfg.exists() {
            if let Ok(s) = std::fs::read_to_string(&cfg) {
                for line in s.lines() {
                    if let Some((k, v)) = line.split_once('=') {
                        if k.trim() == "db_path" {
                            let p = v.trim().trim_matches('"').trim();
                            if !p.is_empty() {
                                return Some(PathBuf::from(p));
                            }
                        }
                    }
                }
            }
        }
    }
    if let Ok(guard) = ctx.workspace_root.lock() {
        if let Some(root) = guard.as_ref() {
            let candidate = root.join("blood.db");
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }
    None
}

fn selected_workspace(ctx: &ToolContext) -> Result<PathBuf, String> {
    ctx.workspace_root
        .lock()
        .map_err(|error| format!("读取工作区状态失败: {error}"))?
        .clone()
        .ok_or_else(|| "请先选择研究工作区。".to_string())
}

fn list_rehab_id_cases(ctx: &ToolContext, limit: usize) -> Result<String, String> {
    let workspace = selected_workspace(ctx)?;
    let mut cases = crate::rehab_context::list_case_summaries(&workspace)?;
    cases.truncate(limit);
    serde_json::to_string_pretty(&json!({
        "source": "workspace_rehab_id_timeline",
        "workspaceScoped": true,
        "caseCount": cases.len(),
        "cases": cases,
    }))
    .map_err(|error| format!("序列化 RehabID 清单失败: {error}"))
}

fn read_rehab_id_case(
    ctx: &ToolContext,
    case_id: &str,
    include_candidates: bool,
    limit: usize,
) -> Result<String, String> {
    if case_id.trim().is_empty() {
        return Err("rehab_case 需要 case_id".into());
    }
    let workspace = selected_workspace(ctx)?;
    let bundle = crate::rehab_context::load_case_bundle(&workspace, case_id)?;
    let candidate_total = bundle
        .observations
        .iter()
        .filter(|item| {
            item.verification_status == crate::rehab_context::VerificationStatus::Candidate
        })
        .count();
    let observations = bundle
        .observations
        .iter()
        .filter(|item| {
            include_candidates
                || item.verification_status == crate::rehab_context::VerificationStatus::Verified
        })
        .take(limit)
        .collect::<Vec<_>>();
    serde_json::to_string_pretty(&json!({
        "source": "workspace_rehab_id_timeline",
        "workspaceScoped": true,
        "caseId": &bundle.case_record.case_id,
        "revision": bundle.revision,
        "demographics": &bundle.case_record.demographics,
        "condition": &bundle.case_record.condition,
        "sources": &bundle.sources,
        "events": &bundle.events,
        "observations": observations,
        "candidateObservationCount": candidate_total,
        "candidatesIncluded": include_candidates,
        "openReviewCount": bundle.cohort_row.open_review_count,
        "cohortStatus": bundle.cohort_row.status,
        "warning": if candidate_total > 0 && !include_candidates {
            "候选观察默认未返回；如需质量审计，可显式设置 include_candidates=true。"
        } else if candidate_total > 0 {
            "候选观察尚未通过质量或人工核验，不得当作确定事实。"
        } else {
            ""
        },
    }))
    .map_err(|error| format!("序列化 RehabID 时间轴失败: {error}"))
}

fn open_readonly(path: &PathBuf) -> Result<Connection, String> {
    Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| format!("无法打开数据库 {}: {e}", path.display()))
}

fn row_to_line(col_names: &[String], row: &rusqlite::Row) -> rusqlite::Result<String> {
    let mut parts = Vec::new();
    for (i, name) in col_names.iter().enumerate() {
        let value = row.get::<usize, Value>(i)?;
        let text = match value {
            Value::Null => "NULL".to_string(),
            Value::Integer(n) => n.to_string(),
            Value::Real(f) => format!("{f:.3}"),
            Value::Text(s) => s,
            Value::Blob(b) => format!("<blob {}B>", b.len()),
        };
        parts.push(format!("{name}={text}"));
    }
    Ok(parts.join(" | "))
}

fn run_select(conn: &Connection, sql: &str, limit: usize) -> Result<Vec<String>, String> {
    let trimmed = sql.trim_start();
    let upper = trimmed.to_uppercase();
    if !upper.starts_with("SELECT") && !upper.starts_with("WITH") && !upper.starts_with("PRAGMA") {
        return Err("只允许只读查询（SELECT / WITH / PRAGMA）".to_string());
    }
    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| format!("SQL 语法错误: {e}"))?;
    let col_names: Vec<String> = (0..stmt.column_count())
        .map(|i| stmt.column_name(i).unwrap_or("?").to_string())
        .collect();
    let mut rows = stmt.query([]).map_err(|e| format!("查询失败: {e}"))?;
    let mut out = Vec::new();
    let mut count = 0;
    while let Some(row) = rows.next().map_err(|e| format!("读取行失败: {e}"))? {
        out.push(row_to_line(&col_names, row).map_err(|e| format!("格式化失败: {e}"))?);
        count += 1;
        if count >= limit {
            break;
        }
    }
    Ok(out)
}

fn list_tables(conn: &Connection) -> Result<Vec<String>, String> {
    run_select(
        conn,
        "SELECT name, (SELECT COUNT(*) FROM sqlite_master) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
        100,
    )
    .map(|rows| {
        let mut out = vec!["表清单：".to_string()];
        out.extend(rows);
        out
    })
}

fn list_athletes(conn: &Connection, keyword: &str, limit: usize) -> Result<Vec<String>, String> {
    let sql = if keyword.trim().is_empty() {
        "SELECT subject_id, name, class, coach, gender, intervention FROM athletes ORDER BY name"
            .to_string()
    } else {
        "SELECT subject_id, name, class, coach, gender, intervention FROM athletes WHERE name LIKE ?1 OR subject_id LIKE ?1 ORDER BY name".to_string()
    };
    let mut stmt = conn.prepare(&sql).map_err(|e| format!("SQL 错误: {e}"))?;
    let col_names: Vec<String> = (0..stmt.column_count())
        .map(|i| stmt.column_name(i).unwrap_or("?").to_string())
        .collect();
    let pattern = format!("%{}%", keyword.trim());
    let mut rows = if keyword.trim().is_empty() {
        stmt.query([]).map_err(|e| format!("查询失败: {e}"))?
    } else {
        stmt.query(params_from_iter([pattern]))
            .map_err(|e| format!("查询失败: {e}"))?
    };
    let mut out = Vec::new();
    let mut count = 0;
    while let Some(row) = rows.next().map_err(|e| format!("读取失败: {e}"))? {
        out.push(row_to_line(&col_names, row).map_err(|e| format!("格式化失败: {e}"))?);
        count += 1;
        if count >= limit {
            break;
        }
    }
    Ok(out)
}

fn athlete_card(conn: &Connection, keyword: &str) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT subject_id, name, class, coach, gender, age, height_cm, weight_kg, intervention \
             FROM athletes WHERE name LIKE ?1 OR subject_id LIKE ?1 LIMIT 1",
        )
        .map_err(|e| format!("SQL 错误: {e}"))?;
    let pattern = format!("%{}%", keyword.trim());
    let athlete = stmt
        .query_row(params_from_iter([pattern.clone()]), |row| {
            let sid: String = row.get(0)?;
            let name: String = row.get(1)?;
            Ok((sid, name))
        })
        .ok();
    let Some((subject_id, name)) = athlete else {
        return Ok(vec![format!("未找到运动员: {keyword}")]);
    };
    let mut out = vec![format!("运动员档案：{name}（{subject_id}）")];
    if let Ok(rows) = run_select(
        conn,
        &format!(
            "SELECT timepoint, analyte, result_value, qualifier, unit FROM blood_markers \
             WHERE subject_id = '{subject_id}' ORDER BY timepoint DESC LIMIT 20"
        ),
        20,
    ) {
        if rows.is_empty() {
            out.push("血指标：无记录".into());
        } else {
            out.push(format!("血指标（最近 {} 条）：", rows.len()));
            out.extend(rows);
        }
    }
    if let Ok(rows) = run_select(
        conn,
        &format!(
            "SELECT fms_total, ybt_left_ant, ybt_right_ant, cmj_height_cm, lactate_pre, cop_left_ellipse_area \
             FROM pretest WHERE subject_id = '{subject_id}'"
        ),
        5,
    ) {
        if rows.is_empty() {
            out.push("前测体能：无记录".into());
        } else {
            out.push("前测体能：".into());
            out.extend(rows);
        }
    }
    if let Ok(rows) = run_select(
        conn,
        &format!(
            "SELECT test_start, device, duration_s FROM cpet_tests \
             WHERE subject_id = '{subject_id}' ORDER BY test_start DESC LIMIT 3"
        ),
        3,
    ) {
        if rows.is_empty() {
            out.push("CPET：无记录".into());
        } else {
            out.push("CPET：".into());
            out.extend(rows);
        }
    }
    if let Ok(rows) = run_select(
        conn,
        &format!(
            "SELECT datetime, session_type, duration_min, avg_hr, max_hr FROM hr_sessions \
             WHERE subject_id = '{subject_id}' ORDER BY datetime DESC LIMIT 10"
        ),
        10,
    ) {
        if rows.is_empty() {
            out.push("心率训练：无记录".into());
        } else {
            out.push(format!("心率训练（最近 {} 条）：", rows.len()));
            out.extend(rows);
        }
    }
    Ok(out)
}

fn blood_panel(
    conn: &Connection,
    analyte: &str,
    athlete: &str,
    limit: usize,
) -> Result<Vec<String>, String> {
    let sql = if athlete.trim().is_empty() {
        "SELECT athlete, subject_id, timepoint, analyte, result_value, qualifier, unit \
         FROM blood_markers WHERE analyte LIKE ?1 ORDER BY timepoint DESC"
            .to_string()
    } else {
        "SELECT athlete, subject_id, timepoint, analyte, result_value, qualifier, unit \
         FROM blood_markers WHERE analyte LIKE ?1 AND (athlete LIKE ?2 OR subject_id LIKE ?2) ORDER BY timepoint DESC".to_string()
    };
    let mut stmt = conn.prepare(&sql).map_err(|e| format!("SQL 错误: {e}"))?;
    let col_names: Vec<String> = (0..stmt.column_count())
        .map(|i| stmt.column_name(i).unwrap_or("?").to_string())
        .collect();
    let a = format!("%{}%", analyte.trim());
    let b = format!("%{}%", athlete.trim());
    let mut rows = if athlete.trim().is_empty() {
        stmt.query(params_from_iter([a]))
            .map_err(|e| format!("查询失败: {e}"))?
    } else {
        stmt.query(params_from_iter([a, b]))
            .map_err(|e| format!("查询失败: {e}"))?
    };
    let mut out = Vec::new();
    let mut count = 0;
    while let Some(row) = rows.next().map_err(|e| format!("读取失败: {e}"))? {
        out.push(row_to_line(&col_names, row).map_err(|e| format!("格式化失败: {e}"))?);
        count += 1;
        if count >= limit {
            break;
        }
    }
    Ok(out)
}

fn week_summary(conn: &Connection, start: &str, end: &str) -> Result<Vec<String>, String> {
    run_select(
        conn,
        &format!(
            "SELECT athlete, COUNT(*) AS 场次, ROUND(SUM(duration_min),1) AS 总时长_min, \
             ROUND(AVG(avg_hr)) AS 平均心率, MAX(max_hr) AS 最大心率 \
             FROM hr_sessions WHERE datetime BETWEEN '{start}' AND '{end}' \
             GROUP BY athlete ORDER BY 总时长_min DESC LIMIT 30"
        ),
        30,
    )
}

pub struct RehabData;

#[async_trait]
impl GalenTool for RehabData {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "rehab_data".into(),
            description: Some(
                "查询当前工作区的治理后 RehabID 时间轴，或兼容查询康复科研数据库 blood.db。\
                 RehabID 操作：list_rehab_cases、rehab_case(case_id, include_candidates)。\
                 数据库操作：list_tables, list_athletes(keyword), athlete_card(athlete), \
                 blood_panel(analyte, athlete可选), cpet_tests(limit), week_summary(start_date,end_date), \
                 query(sql, limit)——query 只允许只读 SELECT。数据库路径由 GALEN_REHAB_DB 环境变量、\
                 ~/.galen/rehab.toml 的 db_path 或工作区 blood.db 决定。结果带数据来源。"
                    .into(),
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "enum": ["list_rehab_cases", "rehab_case", "list_tables", "list_athletes", "athlete_card", "blood_panel", "cpet_tests", "week_summary", "query"]
                    },
                    "case_id": {"type": "string", "description": "伪名化 RehabID"},
                    "include_candidates": {"type": "boolean", "description": "是否包含尚未核验的候选观察；默认 false"},
                    "keyword": {"type": "string", "description": "运动员姓名/ID 关键字"},
                    "athlete": {"type": "string", "description": "运动员姓名/ID"},
                    "analyte": {"type": "string", "description": "血指标名称，如 CK / 睾酮"},
                    "start_date": {"type": "string", "description": "起始日期 YYYY-MM-DD"},
                    "end_date": {"type": "string", "description": "结束日期 YYYY-MM-DD"},
                    "sql": {"type": "string", "description": "只读 SELECT 查询"},
                    "limit": {"type": "integer", "description": "返回行数上限（默认 50，最大 200）"}
                },
                "required": ["operation"]
            }),
        }
    }

    async fn execute(&self, input: Json, ctx: &ToolContext) -> Result<String, String> {
        let operation = input["operation"].as_str().unwrap_or("");
        let limit = (input["limit"].as_u64().unwrap_or(50) as usize).min(MAX_ROWS);
        match operation {
            "list_rehab_cases" => return list_rehab_id_cases(ctx, limit),
            "rehab_case" => {
                return read_rehab_id_case(
                    ctx,
                    input["case_id"].as_str().unwrap_or(""),
                    input["include_candidates"].as_bool().unwrap_or(false),
                    limit,
                )
            }
            _ => {}
        }
        let db_path = resolve_db_path(ctx)
            .ok_or("未找到康复数据库：请设置 GALEN_REHAB_DB 环境变量、~/.galen/rehab.toml 的 db_path，或在工作区放置 blood.db")?;
        let conn = open_readonly(&db_path)?;
        let keyword = input["keyword"].as_str().unwrap_or("");
        let athlete = input["athlete"].as_str().unwrap_or("");
        let analyte = input["analyte"].as_str().unwrap_or("");
        let start = input["start_date"].as_str().unwrap_or("");
        let end = input["end_date"].as_str().unwrap_or("");

        let rows: Vec<String> = match operation {
            "list_tables" => list_tables(&conn)?,
            "list_athletes" => list_athletes(&conn, keyword, limit)?,
            "athlete_card" => athlete_card(&conn, athlete)?,
            "blood_panel" => blood_panel(&conn, analyte, athlete, limit)?,
            "cpet_tests" => run_select(
                &conn,
                &format!("SELECT * FROM cpet_tests ORDER BY test_start DESC LIMIT {limit}"),
                limit,
            )?,
            "week_summary" => {
                if start.is_empty() || end.is_empty() {
                    return Err("week_summary 需要 start_date 和 end_date".into());
                }
                week_summary(&conn, start, end)?
            }
            "query" => {
                let sql = input["sql"].as_str().ok_or("query 操作需要 sql 参数")?;
                run_select(&conn, sql, limit)?
            }
            other => return Err(format!("未知操作: {other}")),
        };

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let empty = rows.is_empty();
        let mut out = vec![
            format!(
                "# 数据来源: {} | 操作: {operation} | 时间: {now}",
                db_path.display()
            ),
            format!("返回 {} 行：", rows.len()),
        ];
        out.extend(rows);
        if empty {
            out.push("（无匹配记录）".into());
        }
        Ok(out.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_workspace() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("galen-rehab-tool-{nonce}"));
        std::fs::create_dir_all(path.join("output")).unwrap();
        path
    }

    #[test]
    fn queries_rehab_db_when_available() {
        let medical = Arc::new(medical_core::MedicalCore::new(None));
        let ctx = ToolContext::new(medical, Mutex::new(None));
        let Some(db_path) = resolve_db_path(&ctx) else {
            eprintln!("跳过：未配置康复数据库");
            return;
        };
        let conn = open_readonly(&db_path).expect("open db");
        let athletes = list_athletes(&conn, "", 5).expect("list athletes");
        assert!(!athletes.is_empty(), "athletes table should have rows");
        let tables = list_tables(&conn).expect("list tables");
        assert!(tables.iter().any(|l| l.contains("athletes")));
        println!("athletes sample: {}", athletes[0]);
    }

    #[tokio::test]
    async fn reads_governed_rehab_id_without_requiring_legacy_database() {
        let workspace = temp_workspace();
        std::fs::write(
            workspace.join("output/normalized.csv"),
            "rehab_id,timepoint,metric,value,unit\nRID-001,baseline,atr,7,deg\n",
        )
        .unwrap();
        std::fs::write(workspace.join("output/quality.json"), "{}").unwrap();
        crate::rehab_context::import_governed_timeline(
            &workspace,
            crate::rehab_context::GovernedTimelineImportInput {
                dataset_path: "output/normalized.csv".into(),
                quality_report_path: "output/quality.json".into(),
                measurements: vec![crate::rehab_context::GovernedMeasurementInput {
                    case_id: "RID-001".into(),
                    timepoint: "baseline".into(),
                    metric: "adams_atrDegrees".into(),
                    value: 7.0,
                    unit: "deg".into(),
                    source_record_id: Some("assessment-a1".into()),
                    verification_status: Some(crate::rehab_context::VerificationStatus::Candidate),
                }],
            },
        )
        .unwrap();
        let medical = Arc::new(medical_core::MedicalCore::new(None));
        let ctx = ToolContext::new(medical, Mutex::new(Some(workspace.clone())));
        let tool = RehabData;

        let default_view = tool
            .execute(
                json!({"operation": "rehab_case", "case_id": "RID-001"}),
                &ctx,
            )
            .await
            .unwrap();
        assert!(default_view.contains("candidateObservationCount\": 1"));
        assert!(!default_view.contains("adams_atrDegrees"));

        let audit_view = tool
            .execute(
                json!({
                    "operation": "rehab_case",
                    "case_id": "RID-001",
                    "include_candidates": true
                }),
                &ctx,
            )
            .await
            .unwrap();
        assert!(audit_view.contains("adams_atrDegrees"));
        assert!(audit_view.contains("assessment-a1"));
        assert!(audit_view.contains("不得当作确定事实"));
        let _ = std::fs::remove_dir_all(workspace);
    }
}
