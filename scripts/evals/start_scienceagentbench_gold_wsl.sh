#!/usr/bin/env bash
set -euo pipefail

RUN_DIR=/opt/galen/evals/runs/scienceagentbench-gold-calibration
PID_FILE="$RUN_DIR/gold-cpu.pid"
LOG_FILE="$RUN_DIR/gold-cpu.combined.log"

if [[ -f "$PID_FILE" ]]; then
  existing_pid=$(cat "$PID_FILE")
  if kill -0 "$existing_pid" 2>/dev/null; then
    echo "already-running:$existing_pid"
    exit 0
  fi
fi

nohup bash /opt/galen/scripts/evals/run_scienceagentbench_gold_wsl.sh \
  >"$LOG_FILE" 2>&1 </dev/null &
pid=$!
echo "$pid" >"$PID_FILE"
echo "started:$pid"
