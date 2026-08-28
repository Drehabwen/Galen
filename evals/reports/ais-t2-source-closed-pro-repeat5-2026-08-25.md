# AIS T2 来源封闭 DeepSeek V4 Pro Repeat-5 可靠性报告

> 由 Galen Eval 自动生成。原始 JSON/JSONL 是事实源，本报告用于审阅与演示。

## Agent 端到端可靠性

| 指标 | 结果 |
|---|---:|
| 运行数 | 30 |
| 硬门通过 | 27/30 |
| 成功率 | 90.0% |
| Wilson 95% 下界 | 74.4% |
| pass^5 | 66.7% |
| Galen Agent Index | 79.7 |
| 总耗时 P50 / P95 | 15548 / 107251 ms |
| TTFR P50 / P95 | 1330 / 2241 ms |
| Token 平均值 | 10081 |
| 证据检索覆盖率 | N/A |
| 证据引用覆盖率 | N/A |
| 禁止证据命中 | 0 |
| 本地 / 外部检索调用 | 0 / 0 |

### 分案例结果

| Case | Model | 通过 | 成功率 | Lower95 | 质量 | 引用覆盖 |
|---|---|---:|---:|---:|---:|---:|
| AIS-C021-T2 | deepseek-v4-pro | 5/5 | 100.0% | 56.6% | 1.000 | N/A |
| AIS-C022-T2 | deepseek-v4-pro | 5/5 | 100.0% | 56.6% | 1.000 | N/A |
| AIS-C023-T2 | deepseek-v4-pro | 3/5 | 60.0% | 23.1% | 0.859 | N/A |
| AIS-C024-T2 | deepseek-v4-pro | 5/5 | 100.0% | 56.6% | 1.000 | N/A |
| AIS-C025-T2 | deepseek-v4-pro | 5/5 | 100.0% | 56.6% | 1.000 | N/A |
| AIS-C026-T2 | deepseek-v4-pro | 4/5 | 80.0% | 37.6% | 0.978 | N/A |

### 硬门失败

- `AIS-C023-T2` run 1 — `required_tool:write_file`：observed=read_file
- `AIS-C023-T2` run 1 — `structured_observation:C023-B-T`：missing id/value/unit tuple: id=C023-B-T, value=Number(9.0), unit=deg
- `AIS-C023-T2` run 1 — `structured_observation:C023-B-L`：missing id/value/unit tuple: id=C023-B-L, value=Number(32.0), unit=deg
- `AIS-C023-T2` run 1 — `causal_boundary`：missing semantic single-case causal/effectiveness limitation
- `AIS-C023-T2` run 1 — `required_artifact:output/ais-c023-t2.md`：missing or empty
- `AIS-C023-T2` run 1 — `previewable_artifact:output/ais-c023-t2.md`：missing or unsupported preview format
- `AIS-C023-T2` run 5 — `required_tool:write_file`：observed=read_file
- `AIS-C023-T2` run 5 — `structured_observation:C023-B-T`：missing id/value/unit tuple: id=C023-B-T, value=Number(9.0), unit=deg
- `AIS-C023-T2` run 5 — `structured_observation:C023-B-L`：missing id/value/unit tuple: id=C023-B-L, value=Number(32.0), unit=deg
- `AIS-C023-T2` run 5 — `causal_boundary`：missing semantic single-case causal/effectiveness limitation
- `AIS-C023-T2` run 5 — `required_artifact:output/ais-c023-t2.md`：missing or empty
- `AIS-C023-T2` run 5 — `previewable_artifact:output/ais-c023-t2.md`：missing or unsupported preview format
- `AIS-C026-T2` run 3 — `structured_observation:C026-B-T`：missing id/value/unit tuple: id=C026-B-T, value=Number(40.0), unit=deg
- `AIS-C026-T2` run 3 — `structured_observation:C026-B-L`：missing id/value/unit tuple: id=C026-B-L, value=Number(22.0), unit=deg
