//! Galen 工作模式系统
//!
//! 三种模式各有独立的 System Prompt，控制 Agent 的行为边界、工具权限和语气风格。

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

const SETTINGS_FILE: &str = "settings.toml";
const MODE_KEY: &str = "mode";

// ---------------------------------------------------------------------------
// Mode enum
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatMode {
    #[default]
    Auto,
    Plan,
    Discuss,
}

impl ChatMode {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Discuss => "讨论",
            Self::Plan => "计划",
            Self::Auto => "自动",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Discuss => "只读顾问 · 检索文献、查询康复数据、追问分析",
            Self::Plan => "制定方案，列出步骤，确认后执行",
            Self::Auto => "自主分解目标，并行执行，汇总产出",
        }
    }

    /// Whether this mode grants write-access (file + command) without confirmation.
    pub fn auto_confirm(&self) -> bool {
        matches!(self, Self::Auto)
    }

    /// Whether write tools are exposed at all in this mode.
    pub fn write_allowed(&self) -> bool {
        matches!(self, Self::Plan | Self::Auto)
    }

    pub fn meta(&self) -> ModeMeta {
        ModeMeta {
            id: format!("{:?}", self).to_lowercase(),
            label: self.label().to_string(),
            description: self.description().to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Mode-specific system prompts
// ---------------------------------------------------------------------------

const DISCUSS_PROMPT: &str = r##"
## 当前模式：讨论

你是 Galen，一个资深医学科研导师。你的工作是像一个经验丰富的同事一样与用户深入讨论研究问题。

### 核心原则
在给出任何建议之前，**先充分理解用户的完整上下文**。每次回复遵循"追问 → 理解 → 建议"的节奏。

### 追问策略

1. **如果用户模糊描述了一个研究方向**（如"我想研究糖尿病"），追问 PICO 中缺失的要素：
   - Population：哪类患者？（年龄、分期、合并症、选择标准）
   - Intervention：什么干预？（具体药物、剂量、疗程、手术方式）
   - Comparison：对比什么？（安慰剂、标准治疗、另一种药物）
   - Outcome：关注什么结局？（主要终点、次要终点、安全性指标）

2. **如果用户要求数据分析**，先确认：
   - 数据来源（自有数据/公共数据库）和样本量
   - 研究设计类型（RCT/队列/病例对照/横断面/荟萃分析）
   - 数据变量和数据字典是否可用
   - 具体的研究假设是什么

3. **如果用户要写论文**，先确认：
   - 目标期刊类型（SCI/中文核心/学位论文/会议摘要）
   - 已有数据和分析结果现状
   - 是否有现成的参考文献库
   - 截止时间和篇幅要求

4. **追问节奏**：
   - 第一轮：最多提 2-3 个最关键的缺失信息
   - 用户回答后再追问下一层
   - 当信息充分时，给出系统性建议

5. **每次回答结构**：
   【理解确认】一句话复述你理解的研究场景
   【需要补充】3-5 个关键问题（如信息不足）
   【方法学建议】基于已知信息的建议（统计方法、文献检索策略等）
   【下一步方向】用户接下来可以做什么

### 行为边界
- 所有只读工具均可使用，包括：
  - 文献检索：search_pubmed, fetch_article, format_citation
  - 工作区读取：read_file, list_files, search_files
  - 康复数据查询：rehab_data（查询对象、量表记录、评估测量、视频/语音资产等，只读，放心使用）
  - 临床案例：analyze_clinical_case
- 不写文件、不执行代码、不改动任何数据
- MCP 工具不在本模式开放
- 如果用户需要写文件或执行分析，引导用户点击顶栏按钮切换到「计划」或「自动」模式

### 语气
专业、耐心、像导师一样的引导式对话。推荐方法时解释背后的统计学原理。
"##;

const PLAN_PROMPT: &str = r##"
## 当前模式：计划

你是 Galen，一个研究方法学家，擅长将模糊的研究想法转化为可执行的研究方案。

### 核心原则
在充分理解用户需求后，**生成结构化的执行计划**。计划必须清晰、可验证，用户确认后才执行。

### 计划生成流程

1. **需求解析**：解读用户目标，补充隐含的前提条件
2. **资源评估**：扫描工作区现有文件（用 list_files / read_file），评估已有数据和分析
3. **方案生成**：输出包含以下结构的计划：

```markdown
# 研究计划：[项目名]

## 目标
[一句话概括最终产出]

## 现有资源
- 数据文件：[列出]
- 已有分析：[列出]
- 参考文献：[列出]

## 执行步骤

| 序号 | 步骤 | 工具/方法 | 依赖 | 预期产出 | 验证标准 |
|------|------|----------|------|---------|---------|
| 1 | ... | ... | - | ... | ... |
| 2 | ... | ... | 步骤1 | ... | ... |

## 关键决策点
- [列出需要用户选择的地方：统计方法选哪种、期刊格式用哪个等]

## 风险和局限
- [数据质量问题、样本量不足、分析方法限制等]
```

4. **并行标注**：标注哪些步骤可以并行执行
5. **确认流程**：计划输出后等待用户确认（"请确认以上方案，我将按顺序执行"）

### 行为边界
- 所有只读工具可用
- write_file / execute_command 可用，但执行前会展示计划并等待确认
- 确认后执行时，每完成一步汇报进展

### 语气
严谨、结构清晰，像在撰写研究方案。关键统计决策列出选项让用户选择。
"##;

const AUTO_PROMPT: &str = r##"
## 当前模式：自动

你是 Galen，一个全栈医学科研 Agent。拿到目标后自主分解、并行执行、汇总产出。

### 核心原则
**拿到目标就动手，不需要反复确认。** 你是用户的研究助理团队，高效执行是你的使命。

### 自治规则

1. **目标分解**：用户给出目标 → 自动分解为子任务（文献检索/数据分析/图表/写作）
2. **并行执行**：独立子任务尽可能并行（如同时检索文献和分析数据）
3. **错误恢复**：遇到错误自动修复，最多尝试 3 次。3 次都失败再向用户报告
4. **进度报告**：每完成一个阶段给出简要进度

### 执行流程

```
收到目标
  │
  ├─ 搜索文献 (search_pubmed) ──→ 保存结果
  ├─ 读取数据 (read_file)    ──→ 理解变量
  │
  ├─ 生成分析脚本 (write_file) ──→ 执行 (execute_command)
  │   └─ 读取输出 ──→ 解读结果
  │
  ├─ 生成图表 (R/Python) ──→ 保存到 output/
  │
  ├─ 撰写稿件 (write_file) ──→ manuscript/
  │
  └─ 排版输出 (typst compile) ──→ PDF
```

### 阶段完成后
主动汇报："已完成文献检索(N篇)、数据分析(基线表+生存分析)、图表(3张)、稿件初稿。需要我进一步修改还是输出 PDF？"

### 行为边界
- 所有工具可用，自动执行，不需确认
- 文件写入到 workspace 内相应的子目录
- 保持 workspace 结构整洁

### 语气
高效直接，像你的研究团队在汇报工作进展。
"##;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Mode metadata (for frontend consumption)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModeMeta {
    pub id: String,
    pub label: String,
    pub description: String,
}

pub fn all_modes() -> Vec<ModeMeta> {
    vec![
        ModeMeta {
            id: "discuss".into(),
            label: ChatMode::Discuss.label().into(),
            description: ChatMode::Discuss.description().into(),
        },
        ModeMeta {
            id: "plan".into(),
            label: ChatMode::Plan.label().into(),
            description: ChatMode::Plan.description().into(),
        },
        ModeMeta {
            id: "auto".into(),
            label: ChatMode::Auto.label().into(),
            description: ChatMode::Auto.description().into(),
        },
    ]
}

pub fn settings_path() -> PathBuf {
    let mut dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    dir.push(".galen");
    dir.push(SETTINGS_FILE);
    dir
}

/// Load the persisted mode, falling back to the default.
pub fn load_mode() -> ChatMode {
    load_mode_from(&settings_path())
}

fn load_mode_from(path: &std::path::Path) -> ChatMode {
    let Ok(content) = std::fs::read_to_string(&path) else {
        return ChatMode::default();
    };
    content
        .lines()
        .find_map(|line| {
            let line = line.trim();
            let (key, value) = line.split_once('=')?;
            if key.trim() != MODE_KEY {
                return None;
            }
            let value = value.trim().trim_matches('"');
            match value {
                "discuss" => Some(ChatMode::Discuss),
                "plan" => Some(ChatMode::Plan),
                "auto" => Some(ChatMode::Auto),
                _ => None,
            }
        })
        .unwrap_or_default()
}

/// Persist the chosen mode so restarts keep the user's preference.
pub fn save_mode(mode: ChatMode) {
    save_mode_to(&settings_path(), mode);
}

fn save_mode_to(path: &std::path::Path, mode: ChatMode) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let key = match mode {
        ChatMode::Discuss => "discuss",
        ChatMode::Plan => "plan",
        ChatMode::Auto => "auto",
    };
    // Read existing file, replace the mode line if present, otherwise append.
    let mut content = std::fs::read_to_string(&path).unwrap_or_default();
    let mut replaced = false;
    let updated = content
        .lines()
        .map(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with(MODE_KEY) && trimmed.contains('=') {
                replaced = true;
                format!("{MODE_KEY} = \"{key}\"")
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    content = if replaced {
        updated
    } else {
        let mut base = if updated.trim().is_empty() {
            String::new()
        } else {
            updated.trim_end().to_string() + "\n"
        };
        base.push_str(&format!("{MODE_KEY} = \"{key}\"\n"));
        base
    };
    let _ = std::fs::write(&path, content);
}

/// Get the full system prompt for a given mode.
/// This is appended to the base MEDICAL_SYSTEM_PROMPT from backend.rs.
pub fn mode_prompt(mode: ChatMode) -> &'static str {
    match mode {
        ChatMode::Discuss => DISCUSS_PROMPT,
        ChatMode::Plan => PLAN_PROMPT,
        ChatMode::Auto => AUTO_PROMPT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_default_is_auto() {
        let mode = ChatMode::default();
        assert_eq!(mode, ChatMode::Auto);
        assert!(mode.auto_confirm());
        assert!(mode.write_allowed());
    }

    #[test]
    fn mode_auto_has_full_permissions() {
        let mode = ChatMode::Auto;
        assert!(mode.auto_confirm());
        assert!(mode.write_allowed());
    }

    #[test]
    fn mode_plan_allows_write_no_auto_confirm() {
        let mode = ChatMode::Plan;
        assert!(!mode.auto_confirm());
        assert!(mode.write_allowed());
    }

    #[test]
    fn mode_prompts_are_non_empty() {
        for mode in [ChatMode::Discuss, ChatMode::Plan, ChatMode::Auto] {
            let prompt = mode_prompt(mode);
            assert!(!prompt.is_empty());
            assert!(
                prompt.len() > 100,
                "Mode {mode:?} prompt too short: {}",
                prompt.len()
            );
        }
    }

    #[test]
    fn mode_persistence_round_trip() {
        let dir = std::env::temp_dir().join(format!("galen-mode-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.toml");

        // Fresh file -> default (Auto)
        assert_eq!(load_mode_from(&path), ChatMode::Auto);

        // Save plan -> loads back as plan
        save_mode_to(&path, ChatMode::Plan);
        assert_eq!(load_mode_from(&path), ChatMode::Plan);

        // Save discuss -> replaces the existing line
        save_mode_to(&path, ChatMode::Discuss);
        assert_eq!(load_mode_from(&path), ChatMode::Discuss);

        // File keeps no duplicate mode keys
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content.matches("mode =").count(), 1);

        std::fs::remove_dir_all(&dir).ok();
    }
}
