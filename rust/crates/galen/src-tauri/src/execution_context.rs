use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

static EXECUTION_CONTEXT_LOCK: Mutex<()> = Mutex::new(());

const CONTINUATION_CUES: &[&str] = &["继续", "接着", "继续做", "继续执行", "任务不变", "可以继续"];
const DISCUSS_CUES: &[&str] = &["先讨论", "停下来讨论", "只讨论", "先别做", "先不要做"];
const INSPECT_CUES: &[&str] = &["先读", "先读取", "先扫描", "只读", "不要修改", "先不要写", "不要写"];
const VERIFY_CUES: &[&str] = &["编译验证", "验证一次", "运行测试", "跑测试", "检查构建"];
const EXECUTE_CUES: &[&str] = &["直接做", "直接修改", "直接生成", "开始执行", "继续执行", "继续完成"];

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum InteractionMode {
    Discuss,
    Inspect,
    Execute,
    Verify,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum VerificationLevel {
    None,
    Short,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct TaskContractState {
    goal: String,
    mode: InteractionMode,
    constraints: Vec<String>,
    forbidden_actions: Vec<String>,
    next_action: Option<String>,
    verification_level: VerificationLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct UserCorrection {
    statement: String,
    created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ExecutionContext {
    version: u32,
    contract: TaskContractState,
    corrections: Vec<UserCorrection>,
}

impl Default for ExecutionContext {
    fn default() -> Self {
        Self {
            version: 1,
            contract: TaskContractState {
                goal: String::new(),
                mode: InteractionMode::Execute,
                constraints: Vec::new(),
                forbidden_actions: Vec::new(),
                next_action: None,
                verification_level: VerificationLevel::Short,
            },
            corrections: Vec::new(),
        }
    }
}

impl ExecutionContext {
    fn advance(&mut self, message: &str) {
        let text = message.trim();
        let continuation = is_continuation(text);
        let mode = infer_mode(text, self.contract.mode);
        let correction = is_correction(text);

        if correction {
            self.corrections.push(UserCorrection {
                statement: bounded(text, 400),
                created_at: now_millis(),
            });
            if self.corrections.len() > 8 {
                self.corrections.drain(..self.corrections.len() - 8);
            }
            self.contract.constraints.push(bounded(text, 240));
            if self.contract.constraints.len() > 6 {
                self.contract
                    .constraints
                    .drain(..self.contract.constraints.len() - 6);
            }
        }

        let mut sticky = self
            .contract
            .forbidden_actions
            .iter()
            .filter(|action| matches!(action.as_str(), "full_build" | "generation"))
            .cloned()
            .collect::<Vec<_>>();
        update_sticky_forbidden(text, &mut sticky);
        if mode == InteractionMode::Discuss {
            sticky.push("tool_calls".to_string());
        }
        if mode == InteractionMode::Inspect {
            sticky.push("writes".to_string());
        }
        sticky.sort();
        sticky.dedup();

        if !continuation || self.contract.goal.is_empty() {
            self.contract.goal = bounded(text, 600);
        }
        self.contract.mode = mode;
        self.contract.forbidden_actions = sticky;
        self.contract.verification_level = if self
            .contract
            .forbidden_actions
            .iter()
            .any(|action| action == "full_build")
        {
            VerificationLevel::None
        } else {
            VerificationLevel::Short
        };
        self.contract.next_action = if mode == InteractionMode::Discuss {
            Some("先讨论并收敛问题".to_string())
        } else if continuation {
            self.contract.next_action.clone()
        } else {
            Some(bounded(text, 400))
        };
    }

    fn render_with_message(&self, message: &str) -> String {
        let constraints = self
            .contract
            .constraints
            .iter()
            .rev()
            .take(4)
            .map(|value| one_line(value, 240))
            .collect::<Vec<_>>()
            .join(" | ");
        let corrections = self
            .corrections
            .iter()
            .rev()
            .take(4)
            .map(|value| one_line(&value.statement, 240))
            .collect::<Vec<_>>()
            .join(" | ");
        format!(
            "## GALEN_EXECUTION_CONTEXT\nsource:backend_authoritative\nmode:{}\ngoal:{}\nverification:{}\nforbidden:{}\nconstraints:{}\nrecent_corrections:{}\nnext_action:{}\n## END_GALEN_EXECUTION_CONTEXT\n\n{}",
            mode_name(self.contract.mode),
            one_line(&self.contract.goal, 600),
            verification_name(self.contract.verification_level),
            self.contract.forbidden_actions.join(","),
            constraints,
            corrections,
            one_line(self.contract.next_action.as_deref().unwrap_or_default(), 400),
            message,
        )
    }
}

pub(crate) fn prepare_model_message(
    workspace: Option<&Path>,
    message: &str,
    update_context: bool,
) -> Result<String, String> {
    let _guard = lock_context()?;
    let mut context = workspace
        .map(load_context)
        .transpose()?
        .unwrap_or_default();
    if update_context {
        context.advance(message);
        if let Some(workspace) = workspace {
            save_context(workspace, &context)?;
        }
    }
    Ok(context.render_with_message(message))
}

fn load_context(workspace: &Path) -> Result<ExecutionContext, String> {
    let path = context_path(workspace);
    if !path.exists() {
        return Ok(ExecutionContext::default());
    }
    let text = std::fs::read_to_string(&path)
        .map_err(|error| format!("读取执行上下文失败 {}: {error}", path.display()))?;
    serde_json::from_str(&text).map_err(|error| format!("解析执行上下文失败: {error}"))
}

fn save_context(workspace: &Path, context: &ExecutionContext) -> Result<(), String> {
    let path = context_path(workspace);
    let parent = path.parent().ok_or("执行上下文路径无父目录")?;
    std::fs::create_dir_all(parent).map_err(|error| format!("创建上下文目录失败: {error}"))?;
    let bytes = serde_json::to_vec_pretty(context)
        .map_err(|error| format!("序列化执行上下文失败: {error}"))?;
    let temporary = path.with_extension("json.tmp");
    let backup = path.with_extension("json.bak");
    std::fs::write(&temporary, bytes).map_err(|error| format!("写入执行上下文失败: {error}"))?;
    if path.exists() {
        if backup.exists() {
            std::fs::remove_file(&backup)
                .map_err(|error| format!("清理执行上下文备份失败: {error}"))?;
        }
        std::fs::rename(&path, &backup)
            .map_err(|error| format!("备份执行上下文失败: {error}"))?;
    }
    if let Err(error) = std::fs::rename(&temporary, &path) {
        if backup.exists() {
            let _ = std::fs::rename(&backup, &path);
        }
        return Err(format!("提交执行上下文失败: {error}"));
    }
    if backup.exists() {
        std::fs::remove_file(&backup)
            .map_err(|error| format!("清理执行上下文备份失败: {error}"))?;
    }
    Ok(())
}

fn context_path(workspace: &Path) -> PathBuf {
    workspace.join(".galen").join("execution-context.json")
}

fn infer_mode(text: &str, fallback: InteractionMode) -> InteractionMode {
    if contains_any(text, DISCUSS_CUES) || text.trim() == "停下来" {
        InteractionMode::Discuss
    } else if contains_any(text, INSPECT_CUES) {
        InteractionMode::Inspect
    } else if contains_any(text, VERIFY_CUES) {
        InteractionMode::Verify
    } else if contains_any(text, EXECUTE_CUES) {
        InteractionMode::Execute
    } else if is_continuation(text) {
        fallback
    } else {
        InteractionMode::Execute
    }
}

fn update_sticky_forbidden(text: &str, forbidden: &mut Vec<String>) {
    if contains_any(text, &["不要编译", "不要一直编译", "别编译", "无需编译", "先不编译"])
        && !forbidden.iter().any(|action| action == "full_build")
    {
        forbidden.push("full_build".to_string());
    }
    if contains_any(text, &["可以编译", "允许编译", "现在编译", "开始编译", "编译验证"])
    {
        forbidden.retain(|action| action != "full_build");
    }
    if contains_any(text, &["不要生成", "别生成"])
        && !forbidden.iter().any(|action| action == "generation")
    {
        forbidden.push("generation".to_string());
    }
    if contains_any(text, &["可以生成", "允许生成", "现在生成"])
    {
        forbidden.retain(|action| action != "generation");
    }
}

fn is_continuation(text: &str) -> bool {
    let normalized = text.trim_matches(|character: char| {
        character.is_whitespace() || matches!(character, '。' | '！' | '!' | '，' | ',')
    });
    CONTINUATION_CUES.contains(&normalized)
}

fn is_correction(text: &str) -> bool {
    contains_any(
        text,
        &["不要", "别再", "先别", "先不要", "任务不变", "我说的是", "直接", "停下来", "继续"],
    )
}

fn contains_any(text: &str, cues: &[&str]) -> bool {
    cues.iter().any(|cue| text.contains(cue))
}

fn bounded(value: &str, limit: usize) -> String {
    value.chars().take(limit).collect()
}

fn one_line(value: &str, limit: usize) -> String {
    bounded(&value.replace(['\r', '\n'], " "), limit)
}

fn mode_name(mode: InteractionMode) -> &'static str {
    match mode {
        InteractionMode::Discuss => "discuss",
        InteractionMode::Inspect => "inspect",
        InteractionMode::Execute => "execute",
        InteractionMode::Verify => "verify",
    }
}

fn verification_name(level: VerificationLevel) -> &'static str {
    match level {
        VerificationLevel::None => "none",
        VerificationLevel::Short => "short",
    }
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

fn lock_context() -> Result<MutexGuard<'static, ()>, String> {
    EXECUTION_CONTEXT_LOCK
        .lock()
        .map_err(|_| "执行上下文存储锁已损坏".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("galen-execution-context-{tag}-{}", std::process::id()))
    }

    #[test]
    fn corrections_persist_across_turns_and_internal_messages_do_not_replace_goal() {
        let root = workspace("persist");
        let first = prepare_model_message(Some(&root), "不要一直编译，直接修改组件", true).unwrap();
        assert!(first.contains("forbidden:full_build"));
        let internal = prepare_model_message(Some(&root), "计划已确认。请执行节点。", false).unwrap();
        assert!(internal.contains("goal:不要一直编译，直接修改组件"));
        assert!(internal.contains("forbidden:full_build"));
        let continued = prepare_model_message(Some(&root), "继续", true).unwrap();
        assert!(continued.contains("goal:不要一直编译，直接修改组件"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn explicit_permission_can_release_a_sticky_build_ban() {
        let root = workspace("release");
        prepare_model_message(Some(&root), "不要编译，先改代码", true).unwrap();
        let released = prepare_model_message(Some(&root), "现在可以编译验证一次", true).unwrap();
        assert!(released.contains("mode:verify"));
        assert!(released.contains("forbidden:\n"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn direct_user_task_replaces_goal_but_inherited_orchestration_does_not() {
        let root = workspace("ownership");
        prepare_model_message(Some(&root), "直接修改巨型组件", true).unwrap();

        let inherited =
            prepare_model_message(Some(&root), "计划已批准，请自动执行下一节点", false).unwrap();
        assert!(inherited.contains("goal:直接修改巨型组件"));

        let replaced = prepare_model_message(Some(&root), "现在实现完整评测体系", true).unwrap();
        assert!(replaced.contains("goal:现在实现完整评测体系"));
        assert!(!replaced.contains("goal:直接修改巨型组件"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn continuation_retains_inspect_mode_and_original_goal() {
        let root = workspace("inspect-continuation");
        prepare_model_message(Some(&root), "先读取现在代码，不要修改", true).unwrap();
        let continued = prepare_model_message(Some(&root), "继续", true).unwrap();

        assert!(continued.contains("mode:inspect"));
        assert!(continued.contains("goal:先读取现在代码，不要修改"));
        assert!(continued.contains("forbidden:writes"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn explicit_generation_permission_releases_only_generation_ban() {
        let root = workspace("generation-release");
        prepare_model_message(Some(&root), "不要生成，也不要编译", true).unwrap();
        let released = prepare_model_message(Some(&root), "现在可以生成", true).unwrap();

        assert!(released.contains("forbidden:full_build"));
        assert!(!released.contains("forbidden:full_build,generation"));
        assert!(released.contains("verification:none"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn workspaces_do_not_share_goals_or_constraints() {
        let first = workspace("isolation-a");
        let second = workspace("isolation-b");
        prepare_model_message(Some(&first), "不要编译，修复项目 A", true).unwrap();
        prepare_model_message(Some(&second), "分析项目 B", true).unwrap();

        let first_view = prepare_model_message(Some(&first), "继续", false).unwrap();
        let second_view = prepare_model_message(Some(&second), "继续", false).unwrap();
        assert!(first_view.contains("goal:不要编译，修复项目 A"));
        assert!(first_view.contains("forbidden:full_build"));
        assert!(second_view.contains("goal:分析项目 B"));
        assert!(second_view.contains("forbidden:\n"));
        assert!(!second_view.contains("项目 A"));

        let _ = std::fs::remove_dir_all(first);
        let _ = std::fs::remove_dir_all(second);
    }
}
