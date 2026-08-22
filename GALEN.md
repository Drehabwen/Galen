# GALEN 项目记忆

格式：日期 | 来源 | 关键发现 | 关联文件

2026-08-14 | 文献检索 PubMed | 有氧运动改善卒中后活动能力：Moncion 2024 BJSM 贝叶斯NMA(PMID 38413134)；Mehta 2012 荟萃(23192710)；Macko 2005 RCT(16151035)；Globas 2012 RCT(21885867)；Peurala 2014(24733289)；Outermans 2015(25573761)。12 周有氧训练 6MWT 组间差约 35-50 m。 | docs/protocol/s01_study_protocol.md 附录A/B

2026-08-14 | 样本量计算 | 主方案 δ=35m, σ=60m, α=0.05, β=0.20, 失访15% → 每组 47 例，含失访每组 56 例，共 112 例；敏感性 72-176 例。 | scripts/sample_size.py

2026-08-14 | 方案设计 s01 完成 | RCT 方案定稿：12 周有氧运动 vs 常规康复，主要终点 6MWT 变化，次要 10m 步速/FAC；中心区组随机+评估者盲法；ITT+ANCOVA；CONSORT 流程图框架已建。plan.json s01=completed。 | docs/protocol/s01_study_protocol.md

下一步：s02 数据采集——设计 CRF 与数据字典；当前工作区无真实试验数据（blood.db 为运动员康复库，非本 RCT 数据源），需人工提供或模拟数据。
