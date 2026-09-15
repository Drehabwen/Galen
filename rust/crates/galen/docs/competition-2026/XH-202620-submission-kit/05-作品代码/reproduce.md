# Galen 复现说明（提交时填写最终版本号）

## 源码位置

- 本地工程：`D:\DEV\Galen-new\rust\crates\galen`
- 提交包中应放入源码压缩包或可访问的仓库快照，并注明提交版本号。

## 构建与运行

1. 在干净环境安装仓库锁定的 Rust/Node 依赖；
2. 按仓库 README 启动 Galen 前端与服务；
3. 打开康复师工作台，导入脱敏队列；
4. 在 Galen 中确认 RehabID 连接器，执行 PI 对话任务；
5. 预览并导出论文 PDF。

## 验证入口

- `D:\DEV\Galen-new\evals\runs\context-memory-probe-20260910.json`
- `D:\DEV\Galen-new\evals\runs\fatigue-scope-probe-20260910-v2.json`
- 技术佐证报告：`06-效果验证报告/Galen-技术佐证报告.pdf`
- 配对基准任务卡：`07-其他材料/paired-benchmark/`（15 张 TOML 卡，已通过 `eval validate`）

## 打包约束

不放入 API 密钥、未脱敏个人数据、浏览器缓存和无关构建产物。模型使用方式写明 ServiceID 或本地模型文件及版本。
