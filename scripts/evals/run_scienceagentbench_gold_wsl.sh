#!/usr/bin/env bash
set -euo pipefail

ROOT=/opt/galen
UPSTREAM="$ROOT/evals/public-benchmarks/ScienceAgentBench"
RUN_DIR="$ROOT/evals/runs/scienceagentbench-gold-calibration"

cd "$UPSTREAM"
exec /opt/scienceagentbench-harness/bin/python -m evaluation.harness.run_evaluation \
  --benchmark_path "$ROOT/evals/public-benchmarks/downloads/benchmark_verified/benchmark" \
  --pred_program_path "$RUN_DIR/pred_programs" \
  --dataset_name /opt/scienceagentbench-data \
  --split train \
  --log_fname "$RUN_DIR/gold-eval.jsonl" \
  --run_id gold-clintox-calibration \
  --cache_level base \
  --max_workers 1 \
  --timeout 1800 \
  --force_rebuild False \
  --clean False \
  --openai_api_key not-used-local-calibration \
  --instance_ids 1
