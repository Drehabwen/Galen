use std::collections::HashSet;

const DATA_TOOLS: &[&str] = &[
    "list_files",
    "read_file",
    "search_files",
    "create_directory",
    "write_file",
    "execute_command",
];
const LITERATURE_TOOLS: &[&str] = &[
    "search_evidence",
    "search_pubmed",
    "fetch_article",
    "format_citation",
    "list_files",
    "read_file",
    "search_files",
    "save_paper",
    "write_file",
];
const FOCUSED_ARTIFACT_TOOLS: &[&str] = &[
    "create_research_plan",
    "list_files",
    "read_file",
    "write_file",
];
const WORKSPACE_TOOLS: &[&str] = &[
    "list_files",
    "read_file",
    "search_files",
    "create_directory",
    "write_file",
];
pub(crate) const READ_WRITE_TOOLS: &[&str] = &["read_file", "write_file"];
pub(crate) const WRITE_ONLY_TOOLS: &[&str] = &["write_file"];
const LOOKUP_TOOLS: &[&str] = &[
    "search_evidence",
    "search_pubmed",
    "fetch_article",
    "format_citation",
    "list_files",
    "read_file",
    "search_files",
];
const REHAB_QUERY_TOOLS: &[&str] = &[
    "search_evidence",
    "rehab_data",
    "list_files",
    "read_file",
    "write_file",
];
pub(crate) const NO_TOOLS: &[&str] = &[];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TaskClass {
    OpenEnded,
    DirectAnswer,
    QuickLookup,
    Literature,
    LocalData,
    Workspace,
    FocusedPlanArtifact,
    ArtifactCreation,
    RehabQuery,
}

#[derive(Debug, Clone)]
pub(crate) struct TaskContract {
    pub(crate) class: TaskClass,
    pub(crate) allowed_tools: Option<&'static [&'static str]>,
    pub(crate) max_tool_turns: u32,
    pub(crate) execution_policy: &'static str,
    pub(crate) artifact_paths: Vec<String>,
    pub(crate) ordered_read_paths: Vec<String>,
    pub(crate) disable_deep_reasoning: bool,
    pub(crate) response_token_cap: Option<u32>,
}

impl TaskContract {
    pub(crate) fn allows_tool(&self, tool_name: &str) -> bool {
        if self
            .allowed_tools
            .is_some_and(|allowed| allowed.contains(&tool_name))
        {
            return true;
        }
        if self.class != TaskClass::Literature {
            return false;
        }
        let Some((server_name, mcp_tool_name)) =
            crate::mcp_client::parse_qualified_tool_name(tool_name)
        else {
            return false;
        };
        crate::tools::research::recognized_mcp_search(server_name, mcp_tool_name).is_some()
    }
}

#[derive(Debug, Default)]
pub(crate) struct WorkingMemory {
    observed_resources: HashSet<String>,
    delivered_artifacts: HashSet<String>,
    pub(crate) consecutive_no_gain_turns: u32,
}

impl WorkingMemory {
    pub(crate) fn observe_tool_result(
        &mut self,
        tool_name: &str,
        input: &serde_json::Value,
        output: &str,
        is_error: bool,
        cache_hit: bool,
    ) -> bool {
        if is_error || cache_hit {
            return false;
        }
        let path = normalize_contract_path(
            input
                .get("path")
                .and_then(|value| value.as_str())
                .unwrap_or_default(),
        );
        match tool_name {
            "write_file" => {
                if !path.is_empty() {
                    self.delivered_artifacts.insert(path.clone());
                }
                let content = input
                    .get("content")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default();
                self.observed_resources
                    .insert(format!("write:{path}:{}", stable_text_hash(content)))
            }
            "read_file" => self.observed_resources.insert(format!("read:{path}")),
            "list_files" | "search_files" => {
                let mut gained = false;
                for line in output.lines().filter(|line| {
                    line.trim_start().starts_with("[FILE]")
                        || line.trim_start().starts_with("[DIR]")
                }) {
                    gained |= self
                        .observed_resources
                        .insert(format!("entry:{}", line.trim()));
                }
                gained
            }
            _ => self
                .observed_resources
                .insert(format!("result:{tool_name}:{}", stable_text_hash(output))),
        }
    }

    pub(crate) fn finish_turn(&mut self, gained_information: bool) {
        if gained_information {
            self.consecutive_no_gain_turns = 0;
        } else {
            self.consecutive_no_gain_turns = self.consecutive_no_gain_turns.saturating_add(1);
        }
    }

    pub(crate) fn delivery_complete(&self, contract: &TaskContract) -> bool {
        !contract.artifact_paths.is_empty()
            && contract
                .artifact_paths
                .iter()
                .all(|path| self.delivered_artifacts.contains(path))
    }
}

fn stable_text_hash(text: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

pub(crate) fn compile_task_contract(
    kind: model_router::TaskKind,
    user_message: &str,
) -> TaskContract {
    let lower = user_message.to_lowercase();
    let literature_task = ["文献", "pubmed", "综述", "证据", "检索"]
        .iter()
        .any(|needle| lower.contains(needle));
    let artifact_paths = extract_artifact_paths(user_message);
    let ordered_read_paths = extract_read_paths(user_message);
    let (class, allowed_tools, max_tool_turns, execution_policy) = if is_discussion_only_task(
        &lower,
    ) {
        (
            TaskClass::OpenEnded,
            Some(NO_TOOLS),
            3,
            "\n\n## 深度讨论契约\n这是不需要工具的分析或复盘任务。保留用户选择的思考强度，不加载工具定义；给出完整、收敛的讨论结论。",
        )
    } else if is_direct_answer_task(&lower) {
        (
            TaskClass::DirectAnswer,
            Some(NO_TOOLS),
            1,
            "\n\n## 快速回答契约\n这是无需检索或工作区操作的直接回答。禁止调用工具；用最短路径给出核心定义、用途和方向性解释，严格遵守用户字数要求。",
        )
    } else if is_explicit_read_write_artifact_task(&lower) {
        (
            TaskClass::ArtifactCreation,
            Some(READ_WRITE_TOOLS),
            4,
            "\n\n## 定点读写契约\n用户已经给出输入与输出路径。严格按照用户声明的顺序读取每个已知路径，不得重排或省略；若用户明确要求验证某个路径的读取失败，也必须且只读取一次，记录错误后继续后续路径。只允许调用 read_file 和 write_file，禁止 list_files、search_files 或目录探索。写入成功后立即总结。",
        )
    } else if is_explicit_rehab_query(&lower) {
        (
            TaskClass::RehabQuery,
            Some(REHAB_QUERY_TOOLS),
            12,
            "\n\n## 本任务数据边界\n仅查询用户明确要求的患者/量表数据；保持只读，返回最小必要字段。",
        )
    } else if is_local_data_task(&lower) && !literature_task {
        (TaskClass::LocalData, Some(DATA_TOOLS), 28, "")
    } else if literature_task {
        (TaskClass::Literature, Some(LITERATURE_TOOLS), 20, "")
    } else if is_focused_plan_artifact_task(&lower) {
        (
            TaskClass::FocusedPlanArtifact,
            Some(FOCUSED_ARTIFACT_TOOLS),
            7,
            "\n\n## 本任务执行预算\n这是边界明确的计划节点交付任务。最多使用 7 轮工具。若用户要求多个研究节点，第一轮必须调用 create_research_plan 写入结构化节点；随后直接调用 write_file 生成用户指定 Artifact。所有文件工具路径必须相对当前工作区，禁止传入工作区绝对路径。只有任务明确依赖某个现有文件时，才按已知相对路径读取一次，禁止先列根目录或用通配符探索。只允许写入用户指定的最终 Artifact，禁止创建辅助脚本或替代产物。若节点输入不足，必须把阻塞原因、已有证据和下一可执行动作写入目标 Artifact，禁止编造缺失数据。write_file 成功后下一轮直接总结，不得再次读取或搜索同一事实。",
        )
    } else if is_explicit_artifact_creation_task(&lower) {
        (
            TaskClass::ArtifactCreation,
            Some(WRITE_ONLY_TOOLS),
            5,
            "\n\n## 本任务交付契约\n用户已经明确要求创建工作区 Artifact，因此不得因缺少非关键背景而停下询问。若研究主题或细节未给出，使用中性占位内容或明确标注的合理假设，并在文档中列出待确认项；先直接调用 write_file 生成用户指定路径，再依据写入结果确认非空并立即总结。不要在写入前反复列目录、读取不存在的记忆文件或要求用户回复“用示例”。",
        )
    } else if is_workspace_artifact_task(&lower) {
        (TaskClass::Workspace, Some(WORKSPACE_TOOLS), 16, "")
    } else if matches!(kind, model_router::TaskKind::QuickLookup) {
        (TaskClass::QuickLookup, Some(LOOKUP_TOOLS), 8, "")
    } else {
        (TaskClass::OpenEnded, None, 28, "")
    };
    let disable_deep_reasoning = matches!(
        class,
        TaskClass::DirectAnswer | TaskClass::FocusedPlanArtifact | TaskClass::ArtifactCreation
    );
    let response_token_cap = Some(match class {
        TaskClass::DirectAnswer => 768,
        TaskClass::QuickLookup => 1_200,
        TaskClass::FocusedPlanArtifact | TaskClass::ArtifactCreation => 1_200,
        TaskClass::LocalData | TaskClass::Workspace | TaskClass::RehabQuery => 1_800,
        TaskClass::Literature => 2_600,
        TaskClass::OpenEnded if allowed_tools == Some(NO_TOOLS) => 2_400,
        TaskClass::OpenEnded => 3_072,
    });
    TaskContract {
        class,
        allowed_tools,
        max_tool_turns,
        execution_policy,
        artifact_paths,
        ordered_read_paths,
        disable_deep_reasoning,
        response_token_cap,
    }
}

pub(crate) fn task_execution_policy(user_message: &str) -> String {
    compile_task_contract(
        model_router::TaskKind::from_intent(user_message),
        user_message,
    )
    .execution_policy
    .to_string()
}

pub(crate) fn is_workspace_artifact_task(text: &str) -> bool {
    [
        "读取",
        "写入",
        "文件",
        "工作区",
        "output/",
        "output\\",
        "节点",
        "计划",
        "产物",
        "artifact",
        ".md",
        ".json",
        ".toml",
    ]
    .iter()
    .any(|needle| text.contains(needle))
}

fn is_direct_answer_task(text: &str) -> bool {
    let direct_cue = [
        "直接回答",
        "简要回答",
        "简短回答",
        "不超过",
        "用一句话",
        "无需检索",
    ]
    .iter()
    .any(|needle| text.contains(needle));
    let external_evidence_cue = [
        "检索",
        "搜索",
        "查找",
        "查一下",
        "最新",
        "文献",
        "综述",
        "证据",
        "pubmed",
        "读取",
        "工作区",
    ]
    .iter()
    .any(|needle| text.contains(needle));
    direct_cue && !external_evidence_cue
}

fn is_discussion_only_task(text: &str) -> bool {
    let explicitly_no_tools = ["不要调用工具", "不调用工具", "无需工具", "不使用工具"]
        .iter()
        .any(|needle| text.contains(needle));
    let discussion_cue = [
        "深入讨论",
        "深度讨论",
        "重点讨论",
        "方案复盘",
        "分析利弊",
        "批判性分析",
    ]
    .iter()
    .any(|needle| text.contains(needle));
    let requires_external_context = [
        "检索",
        "搜索",
        "查找",
        "最新",
        "pubmed",
        "读取",
        "工作区",
        "写入",
        "生成文件",
        "output/",
        "output\\",
    ]
    .iter()
    .any(|needle| text.contains(needle));
    explicitly_no_tools || (discussion_cue && !requires_external_context)
}

pub(crate) fn is_focused_plan_artifact_task(text: &str) -> bool {
    let plan_or_node = ["计划", "节点", "plan.json", "node"]
        .iter()
        .any(|needle| text.contains(needle));
    let explicit_delivery = ["写入", "生成", "保存", "output/", "output\\", "artifact"]
        .iter()
        .any(|needle| text.contains(needle));
    plan_or_node && explicit_delivery
}

pub(crate) fn is_explicit_artifact_creation_task(text: &str) -> bool {
    let create_intent = ["创建", "生成", "写入", "保存"]
        .iter()
        .any(|needle| text.contains(needle));
    let artifact_path = ["output/", "output\\", "artifact", ".md", ".json", ".csv"]
        .iter()
        .any(|needle| text.contains(needle));
    create_intent && artifact_path
}

fn is_explicit_read_write_artifact_task(text: &str) -> bool {
    let read_intent = ["读取", "读入", "根据", "基于"]
        .iter()
        .any(|needle| text.contains(needle));
    read_intent
        && is_explicit_artifact_creation_task(text)
        && extract_path_mentions(text).len() >= 2
}

fn extract_path_mentions(text: &str) -> Vec<&str> {
    text.split_whitespace()
        .map(|token| {
            token.trim_matches(|character: char| {
                matches!(
                    character,
                    '，' | '。' | '；' | '：' | ',' | '.' | ';' | ':' | '"' | '\'' | '“' | '”'
                )
            })
        })
        .filter(|token| {
            (token.contains('/') || token.contains('\\'))
                && [".md", ".json", ".csv", ".toml", ".txt"]
                    .iter()
                    .any(|extension| token.contains(extension))
        })
        .collect()
}

fn extract_read_paths(text: &str) -> Vec<String> {
    let mut paths = Vec::new();
    for token in extract_path_mentions(text) {
        let Some(end) = [".md", ".json", ".csv", ".toml", ".txt"]
            .iter()
            .filter_map(|extension| token.find(extension).map(|index| index + extension.len()))
            .min()
        else {
            continue;
        };
        let path = normalize_contract_path(&token[..end]);
        if !path.starts_with("output/") && !paths.contains(&path) {
            paths.push(path);
        }
    }
    paths
}

fn extract_artifact_paths(text: &str) -> Vec<String> {
    let mut paths = Vec::new();
    for (start, _) in text
        .match_indices("output/")
        .chain(text.match_indices("output\\"))
    {
        let tail = &text[start..];
        let end = tail
            .find(|ch: char| {
                ch.is_whitespace()
                    || matches!(ch, '，' | '。' | '；' | ';' | ',' | ')' | '）' | ']' | '】')
            })
            .unwrap_or(tail.len());
        let path = normalize_contract_path(
            tail[..end].trim_matches(|ch: char| matches!(ch, '`' | '"' | '\'' | ':' | '：')),
        );
        if !path.is_empty() && !paths.contains(&path) {
            paths.push(path);
        }
    }
    paths
}

pub(crate) fn normalize_contract_path(path: &str) -> String {
    path.replace('\\', "/").trim_start_matches("./").to_string()
}

pub(crate) fn is_explicit_rehab_query(text: &str) -> bool {
    let mentions_subject_data = [
        "患者",
        "受试者",
        "量表",
        "评估记录",
        "测量记录",
        "视频资产",
        "语音资产",
        "康复数据库",
        "rehab_data",
    ]
    .iter()
    .any(|needle| text.contains(needle));
    let asks_to_query = ["查询", "查找", "读取", "检索", "列出", "统计"]
        .iter()
        .any(|needle| text.contains(needle));
    mentions_subject_data && asks_to_query
}

pub(crate) fn is_local_data_task(text: &str) -> bool {
    let lower = text.to_lowercase();
    ["数据", "肌电", "emg", "csv", "统计", "回归", "脚本"]
        .iter()
        .any(|needle| lower.contains(needle))
}

#[cfg(test)]
pub(crate) fn max_tool_turns_for_task(user_message: &str) -> u32 {
    compile_task_contract(
        model_router::TaskKind::from_intent(user_message),
        user_message,
    )
    .max_tool_turns
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literature_contract_allows_only_recognized_qualified_mcp_searches() {
        let contract = compile_task_contract(
            model_router::TaskKind::Chat,
            "请检索脑卒中康复的中文文献",
        );

        assert_eq!(contract.class, TaskClass::Literature);
        assert!(contract.allows_tool("mcp__cnki__cnki_structured_search"));
        assert!(!contract.allows_tool("mcp__unrelated__search_papers"));
    }
}
