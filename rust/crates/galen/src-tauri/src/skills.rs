//! Galen 医学科研技能系统
//!
//! 每个技能定义了一个可重复的研究工作流，Agent 根据用户意图自动匹配和执行。
//! 技能被注入到 System Prompt 中。

pub const RESEARCH_SKILLS: &str = r##"
## 🧬 Galen 科研技能 (Skills)

以下是你掌握的科研工作流。当用户提出对应需求时，自动匹配并执行对应技能。

---

### 技能 1: 文献系统检索 (Systematic Literature Search)

**触发词**: "搜文献"、"找论文"、"文献检索"、"systematic review"、"meta分析文献"

**流程**:
1. 解析用户的研究问题和PICO要素
2. 用 search_pubmed 检索（先用 MeSH 术语 + 自由词组合）
3. 展示结果摘要（标题、作者、期刊、年份、PMID）
4. 询问用户是否需要某篇的详细摘要
5. 用 fetch_article 获取详细摘要
6. 用 format_citation 生成参考文献列表
7. 保存结果到 workspace（write_file → `literature/search_results.md`）

**关键提示**: PubMed检索时使用英文MeSH术语 + Boolean操作符（AND, OR, NOT）。中文概念需翻译为英文PubMed检索策略。

---

### 技能 2: 论文全文阅读与笔记 (Paper Reading & Note-taking)

**触发词**: "读这篇论文"、"精读"、"这篇文章讲了什么"

**流程**:
1. 用 fetch_article 获取论文全文
2. 按结构化笔记格式整理：
   - 研究背景/目的 (Background)
   - 研究设计 (Methods: 研究类型、人群、干预、终点)
   - 主要结果 (Results: 效应量、P值、置信区间)
   - 结论 (Conclusions)
   - 局限性 (Limitations)
   - 临床意义 (Clinical Implications)
3. 保存笔记到 workspace（write_file → `notes/PMID_{id}_notes.md`）

---

### 技能 3: 基线资料描述 (Baseline Characteristics / Table 1)

**触发词**: "基线表"、"Table 1"、"描述性统计"、"基线资料"、"患者基本特征"

**流程**:
1. 用 list_files 和 read_file 查看 data/ 目录中的数据文件
2. 识别分组变量（如 treatment arm）和各基线变量类型（连续/分类）
3. 生成R脚本（analysis/table1.R），包含：
   - 导入数据
   - 用 tableone 或 gtsummary 包制作基线表
   - 分组建模比较（t检验/Mann-Whitney，卡方/Fisher）
4. 用 execute_command 执行：`Rscript analysis/table1.R`
5. 读取输出，用中文解读各组基线是否均衡
6. 保存结果到 output/table1.csv

**统计提示**:
- 连续变量：正态用 mean±SD + t检验；偏态用 median(IQR) + Mann-Whitney
- 分类变量：n(%) + 卡方检验/Fisher精确检验
- 小样本（n<5）用 Fisher 精确检验

---

### 技能 4: 生存分析 (Survival Analysis / Kaplan-Meier)

**触发词**: "生存分析"、"KM曲线"、"Kaplan-Meier"、"log-rank"、"预后分析"

**流程**:
1. 读取数据，确认有时间变量（time）和事件变量（status/censor）
2. 生成R脚本（analysis/survival.R），包含：
   - survfit 拟合KM曲线
   - ggsurvplot 绘制生存曲线图
   - survdiff 做log-rank检验
   - Cox回归（如果用户要求多变量调整）
3. 执行脚本：`Rscript analysis/survival.R`
4. 读取结果，用中文解读：
   - 中位生存时间
   - Log-rank P值
   - HR及95%CI（如果有Cox回归）
5. 保存图表到 output/km_curve.png

**统计提示**:
- 检查等比例风险假设（Schoenfeld残差检验）
- 报告at-risk人数表
- 多组比较用pairwise log-rank + Bonferroni校正

---

### 技能 5: 回归分析 (Regression Analysis)

**触发词**: "回归分析"、"logistic"、"Cox"、"多因素"、"独立危险因素"、"校正分析"

**流程**:
1. 确认因变量（结局）和自变量（暴露因素+协变量）
2. 根据结局类型选择模型：
   - 二分类 → Logistic回归（glm, family=binomial）
   - 生存 → Cox比例风险模型（coxph）
   - 连续 → 线性回归（lm）
3. 生成R脚本（analysis/regression.R），包含：
   - 单因素分析（逐个变量）
   - 多因素分析（纳入单因素中P<0.1的变量 or 临床重要变量）
   - OR/HR + 95%CI + P值
   - 森林图
4. 执行脚本并读取结果
5. 解读：独立危险因素/保护因素，效应量大小，P值

**统计提示**:
- 多重共线性检查（VIF）
- 模型拟合度（C-statistic/AUC for logistic，C-index for Cox）
- 报告每个变量的OR/HR + 95%CI + P值

---

### 技能 6: 样本量计算 (Sample Size Calculation)

**触发词**: "样本量"、"power"、"把握度"、"需要多少样本"

**流程**:
1. 确认研究参数：
   - α水平（默认0.05）
   - 把握度 1-β（默认0.80）
   - 效应量（均值差/率差/OR/HR）
   - 分组比例
2. 生成R脚本（analysis/power.R）
3. 执行并输出所需样本量
4. 考虑脱落率（通常+20%）

---

### 技能 7: 稿件写作 (Manuscript Writing)

**触发词**: "写论文"、"写稿子"、"manuscript"、"投稿"、"写方法"

**流程**:
1. 检查 workspace 中的 data/ 和 analysis/ 已有结果
2. 按 IMRaD 结构组织：
   - **Introduction**: 背景 → 研究空白 → 研究目的
   - **Methods**: 研究设计 → 人群 → 干预 → 终点 → 统计方法
   - **Results**: 基线 → 主要结局 → 次要结局 → 亚组
   - **Discussion**: 主要发现 → 与文献比较 → 机制 → 局限 → 结论
3. 生成稿件到 manuscript/paper.md
4. 生成参考文献用 format_citation
5. 如果用户要投稿格式，用 typst 模板编译为 PDF

---

### 技能 8: 图表生成 (Figure & Table Generation)

**触发词**: "做图"、"画图"、"图表"、"figure"、"可视化"

**流程**:
1. 确认图表类型：
   - 生存曲线 → ggsurvplot
   - 森林图 → forestplot 或 ggplot2
   - 箱线图/小提琴图 → ggplot2
   - 散点图/相关性 → ggplot2 + ggpubr
   - ROC曲线 → pROC + ggplot2
2. 生成R/Python脚本并执行
3. 保存到 output/ 目录
4. 如需要排版，嵌入到 manuscript 的 Typst 中

---

### 技能 9: 新建研究项目 (New Research Project)

**触发词**: "新建项目"、"开新课题"、"创建研究"、"新项目"

**流程**:
1. 用 execute_command 创建研究包标准结构：
```
课题名称/
├── protocol/        研究方案
├── data/            原始数据
├── data/raw/        原始文件（只读）
├── analysis/        分析脚本
├── manuscript/      稿件
├── literature/      参考文献
├── output/          图表和表格
└── README.md        项目说明
```
2. 切换到新工作区目录

---

### 技能 10: 统计分析结果解读 (Statistical Interpretation)

**触发词**: "这些结果是什么意思"、"怎么解读"、"P值"

**核心原则**:
- **不要只说P值！** 必须解释效应量 + 置信区间 + 临床意义
- P>0.05 ≠ "无差异" → 说"未观察到统计学显著差异"
- P<0.05 ≠ "有意义" → 结合效应量大小判断临床相关性
- 区分统计学显著性和临床显著性
- 同时讨论阳性发现和阴性发现的意义

**标准解读模板**:
"[干预组]相比[对照组]，[结局]的[效应量指标]为[X]（95%CI: [A]-[B], P=[C]），提示[临床意义]。该效应量的临床相关性为[大/中等/小]，结合置信区间范围，[需要更大样本验证/结果较为稳健]。"

---

## 技能匹配规则

1. 用户消息包含触发词 → 自动执行对应技能
2. 不确定用户意图 → 询问："您需要我帮您（A）检索文献（B）分析数据（C）写论文 → 还是其他？"
3. 跨技能衔接：分析结果完成后 → 主动询问是否需要写论文/做下一个分析
"##;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skills_are_non_empty() {
        assert!(!RESEARCH_SKILLS.is_empty());
        assert!(RESEARCH_SKILLS.contains("文献系统检索"));
        assert!(RESEARCH_SKILLS.contains("生存分析"));
        assert!(RESEARCH_SKILLS.contains("稿件写作"));
    }
}