#!/usr/bin/env bash
set -euo pipefail

agent=${1:?usage: run_scienceagentbench_agent_cpu_wsl.sh codex|claude}
case "$agent" in
  codex|claude) ;;
  *) echo "unsupported agent: $agent" >&2; exit 2 ;;
esac

ROOT=/opt/galen
UPSTREAM="$ROOT/evals/public-benchmarks/ScienceAgentBench"
RUN_DIR="$ROOT/evals/runs/scienceagentbench-clintox-cpu/$agent"
PRED_DIR="$ROOT/evals/runs/scienceagentbench-after-download/pred_programs/$agent"

mkdir -p "$RUN_DIR"
cd "$UPSTREAM"
exec /opt/scienceagentbench-harness/bin/python -m evaluation.harness.run_evaluation \
  --benchmark_path "$ROOT/evals/public-benchmarks/downloads/benchmark_verified/benchmark" \
  --pred_program_path "$PRED_DIR" \
  --dataset_name /opt/scienceagentbench-data \
  --split train \
  --log_fname "$RUN_DIR/eval.jsonl" \
  --run_id "cpu-clintox-$agent" \
  --cache_level base \
  --max_workers 1 \
  --timeout 1800 \
  --force_rebuild False \
  --clean False \
  --openai_api_key not-used-local-calibration \
  --instance_ids 1
