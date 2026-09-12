# 03—作品 Demo

## 一、Demo 体验地址

### 公开源码与文档

- **Galen GitHub 仓库：** <https://github.com/Drehabwen/Galen>
- **当前开发分支：** <https://github.com/Drehabwen/Galen/tree/galen-research-workbench>
- **最新公开桌面版 Release：** <https://github.com/Drehabwen/Galen/releases/tag/v0.1.3>
- **Release 列表：** <https://github.com/Drehabwen/Galen/releases>

当前没有稳定的公共在线 Demo 地址，因此不虚构在线链接。本作品采用“GitHub 源码 + 选手自行部署 + 交互视频”的提交方式。评委可从 Release 下载 Windows 安装包，也可以按下方步骤从源码运行。

### 本地源码位置

```text
D:\DEV\Galen-new\rust\crates\galen
```

### 提交包中的交互视频

`Galen-RehabID-完整队列到正式论文-v5.mp4`

视频实测：32.32 秒，1600×900，25 fps。它展示真实 Galen 界面中的连接器、RehabID 队列、研究任务状态、分析过程和正式 PDF 全文预览。

## 二、选手自行部署

### 方式 A：Windows Release

1. 打开 [Galen v0.1.3 Release](https://github.com/Drehabwen/Galen/releases/tag/v0.1.3)。
2. 下载 Windows `Galen_0.1.0_x64-setup.exe` 安装包。
3. 启动后配置自己的 DeepSeek API Key 和工作区。
4. 在主对话输入研究任务，确认计划后开始执行。

### 方式 B：从源码运行

```bash
git clone --branch galen-research-workbench https://github.com/Drehabwen/Galen.git
cd Galen/rust/crates/galen
npm ci
npm run tauri dev
```

构建前置条件和 Windows/macOS 打包方法见：

- `docs/DEVELOPER_ONBOARDING.md`
- `docs/GALEN_USER_GUIDE.md`
- 仓库根目录 `README.md`

API Key 由体验者自行配置，不能把个人 Key 写入安装包、视频或提交材料。

## 三、现场演示顺序

1. 在康复师工作台中打开脱敏队列。
2. Galen Connector 发现并预览 RehabID、评估时点和核心观察。
3. 确认导入，数据进入当前研究的纵向时间轴。
4. 在 PI 对话中发起数据质检和恢复轨迹分析。
5. Galen 生成论文级 PDF，并在应用内完成全文预览。

## 四、作品说明

Galen 不是单一的聊天机器人，而是面向康复科研的数据治理与执行工作台。它把康复师工作台、量表、评估记录和多时间点 RehabID 数据接入同一个 ResearchProject，再由 PI Agent 组织清洗、分析、证据追溯和成果交付。

本 Demo 的首发场景是运动疲劳多模态纵向研究，展示的最小闭环为：

```text
研究任务输入
  → Connector 获取脱敏 RehabID 队列
  → 数据质量检查与时间对齐
  → PI Agent 生成分析任务
  → 结果、引用和证据回流
  → 正式 PDF 生成与全文预览
```

## 五、提交材料对应关系

| 材料 | 位置 |
|---|---|
| Demo 体验地址 | 本页 GitHub 仓库 / Release 链接 |
| 文档说明 | 本 README、`docs/GALEN_USER_GUIDE.md` |
| 源码 | GitHub `galen-research-workbench` 分支 |
| 交互视频 | 本目录 `Galen-RehabID-完整队列到正式论文-v5.mp4` |
| 技术佐证 | `06-效果验证报告/` |
| 研究方案与计划书 | `04-作品方案/` |

视频应保持在 3 分钟以内，清晰展示从任务输入到结果输出的完整流程；不得用 PPT 代替应用交互，也不得只提供配音解说。
