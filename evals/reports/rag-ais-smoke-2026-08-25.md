# Galen AIS RAG 组件测评报告

> 由 Galen Eval 自动生成。原始 JSON/JSONL 是事实源，本报告用于审阅与演示。

## RAG 组件基准

- 数据集：`rag-ais-scoliosis-v1`（hash `18f983001d759393`）
- 引擎：`tantivy-bm25` / `jieba-search-mode`
- Git：`42a7879b1908a0cd79564df7a0cb1df26b77819b-dirty`
- 结论：**PASS**

| 指标 | 结果 |
|---|---:|
| Recall@3 | 1.000 |
| Precision@3 | 0.333 |
| MRR | 1.000 |
| nDCG@3 | 1.000 |
| 域外空结果准确率 | 1.000 |
| 禁止证据命中 | 0 |
| 热检索 P50 / P95 | 3.0 / 3.0 ms |
| 冷建索引 | 978 ms |

### RAG 硬门

- [x] `recall_at_k`：1.000 >= 1.000
- [x] `mrr`：1.000 >= 0.900
- [x] `ndcg_at_k`：1.000 >= 0.900
- [x] `latency_p95_ms`：3.0 <= 100 ms
- [x] `cold_index_ms`：978 <= 2000 ms
- [x] `zero_forbidden_hits`：forbidden_hits=0
- [x] `negative_query_accuracy`：1.000 >= 1.000

### 逐查询排名

| Query | 类型 | Recall | RR | nDCG | 返回 Evidence |
|---|---|---:|---:|---:|---|
| ais-schroth | 正查询 | 1.000 | 1.000 | 1.000 | ev-ais-schroth<br>ev-ais-monitoring<br>ev-ais-brace |
| ais-brace | 正查询 | 1.000 | 1.000 | 1.000 | ev-ais-brace<br>ev-ais-schroth<br>ev-ais-respiratory |
| ais-quality-of-life | 正查询 | 1.000 | 1.000 | 1.000 | ev-ais-srs22<br>ev-ais-schroth<br>ev-ais-respiratory |
| ais-respiratory | 正查询 | 1.000 | 1.000 | 1.000 | ev-ais-respiratory<br>ev-ais-schroth<br>ev-ais-monitoring |
| ais-monitoring | 正查询 | 1.000 | 1.000 | 1.000 | ev-ais-monitoring<br>ev-ais-schroth<br>ev-ais-respiratory |
| ood-parkinson | 域外负查询 | 0.000 | 0.000 | 0.000 | — |
| ood-diabetic-foot | 域外负查询 | 0.000 | 0.000 | 0.000 | — |
