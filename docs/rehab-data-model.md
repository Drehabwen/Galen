# 闭环康复科研 · 统一数据模型

> 目标：让量表的文字、评估的数字、视频的动作、语音的记录——四种模态的数据，统一挂接到同一个对象（`subject_id` → 评估会话）上，支撑 Agent 跨模态查询、证据可追溯、报告可复现。

**状态**：v0.1 草案（数据平面主干设计）

---

## 一、为什么需要统一模型

康复科研数据天然多模态：

| 模态 | 示例 | 源头形态 | 管道产物 |
|---|---|---|---|
| 量表 | FMS、SRS-22、VAS、问卷 | 表单录入 | 结构化得分 |
| 评估 | ROM、CMJ、CPET、手法评估 | 测量记录 | 指标 + 单位 |
| 视频 | 动作、步态、运动执行 | 视频文件 | 姿态/角度/对称性/负荷 |
| 语音 | 接诊、病史、随访口述 | 录音文件 | 转写 + 结构化笔记 |

如果四类数据各存各的，一份需要同时引用四类证据的康复报告就无法闭环。**统一模型的本质：所有模态都挂在 `subject_id → assessment_session` 下，最终都变成证据链上的节点。**

---

## 二、核心实体

```text
subject（对象）
  └── assessment_session（评估会话）
        ├── scale_record    量表记录
        ├── measure_record  评估测量
        ├── video_asset     视频（含派生 pose_result）
        └── audio_asset     语音（含派生 transcript / note）
             └── 全部可挂到 evidence（证据链）
```

### 1. subject（对象主表）

| 字段 | 说明 |
|---|---|
| `subject_id` | 唯一业务主键（对应现有 `athletes.subject_id`） |
| `name` / `aliases` | 姓名与别名（对齐现有姓名归一口径） |
| `class` / `coach` / `gender` / `dob` | 组别、教练、性别、出生日期 |
| `intervention` | 干预分组 |

### 2. assessment_session（评估会话）

| 字段 | 说明 |
|---|---|
| `session_id` | 会话主键（一次评估/一次随访/一次测试） |
| `subject_id` | 挂接对象 |
| `date` / `scene` | 时间、场景（筛查/随访/训练后） |
| `operator` / `device` | 操作者、设备 |
| `status` | 采集/处理中/已完成/已签核 |

### 3. scale_record（量表记录）

| 字段 | 说明 |
|---|---|
| `session_id` / `subject_id` | 挂接 |
| `form_id` / `form_version` | 表单定义 + 版本（FMS 2026-01 等） |
| `item_id` / `item_label` | 条目 |
| `score` / `unit` | 得分与单位 |
| `scoring_rule_version` | 计分规则版本（可复现） |

### 4. measure_record（评估测量）

| 字段 | 说明 |
|---|---|
| `session_id` / `subject_id` | 挂接 |
| `protocol_id` / `protocol_version` | 测量协议 + 版本 |
| `metric` / `value` / `unit` | 指标名、值、单位 |
| `laterality` / `timepoint` | 左右侧、时间点（BL/24H/72H 等） |

### 5. video_asset（视频 + 派生）

原始层：

| 字段 | 说明 |
|---|---|
| `session_id` / `subject_id` | 挂接 |
| `file_path` / `capture_device` | 原始文件路径、设备 |
| `duration` / `format` | 时长、格式 |

派生层（pose_result，可重建）：

| 字段 | 说明 |
|---|---|
| `joints_3d` / `com_3d` | 三维关节、质心（对应 `gymnasts_deep.json`） |
| `angles` / `symmetry` / `loads` | 角度、对称性、关节负荷 |
| `model_version` / `processing_log` | 姿态模型版本、处理日志（血缘） |

### 6. audio_asset（语音 + 派生）

原始层：

| 字段 | 说明 |
|---|---|
| `session_id` / `subject_id` | 挂接 |
| `file_path` / `duration` | 原始录音路径、时长 |

派生层：

| 字段 | 说明 |
|---|---|
| `transcript` | ASR 转写原文 |
| `structured_note` | 结构化病历/笔记（对应 MedVoice 产物） |
| `asr_model_version` | 识别模型版本 |

---

## 三、血缘与版本（科研底线）

每条记录（含派生）必须带：

```text
source_file  = 原始文件（视频/录音/照片/录入批次）
operator     = 谁录入/谁处理
device       = 哪台设备
created_at   = 入库时间
processing_log = 处理脚本与参数（JSON）
schema_version = 数据结构版本
```

**原则**：

1. **原始文件永远保留**，派生数据（姿态、转写、指标）可重建
2. 任何结论都能倒查：报告 → 证据 → 指标 → 原始文件
3. 清洗规则（单位统一、去重、异常标记）记录在 processing_log，可复现

---

## 四、与现有资产的映射

| 现有资产 | 对应模型位置 |
|---|---|
| `blood.db`（athletes/pretest/cpet/hr） | subject + measure_record + assessment_session |
| `blood_markers` | measure_record（时间点 + 指标长表） |
| 平板训练录入（localStorage → JSON → 入库） | 采集端 → scale_record / measure_record |
| MedVoice（语音转病历） | audio_asset → transcript / structured_note |
| 姿态捕捉（4D-Humans / RTMPose / loads） | video_asset → pose_result |
| 统一数据集 / 完整性矩阵 | 管道校验层（接入时复用） |

---

## 五、Agent 接入（Galen 闭环）

- `rehab_data` 工具（blood.db 结构化切片）已接入，结果带数据来源头
- 后续扩展查询操作：按 `session_id` 查全部模态、按 `subject_id` 跨模态聚合
- 证据链节点 = `模态 + 指标 + 来源`，报告引用时自动可追溯

---

## 六、落地顺序

1. ✅ **结构化切片**：blood.db 查询工具（已完成）
2. ⏳ **量表契约**：表单定义 + 计分规则 + 录入校验（源头拦截坏数据）
3. ⏳ **视频管道**：采集规范 → 姿态处理 → pose_result 入库（带血缘）
4. ⏳ **语音管道**：录音规范 → ASR → structured_note 入库（带血缘）
5. ⏳ **跨模态查询**：rehab_data 扩展按会话/对象聚合四模态

---

*本文档是数据平面主干设计，与 `docs/GALEN_USER_GUIDE.md`（使用说明）配套。*
