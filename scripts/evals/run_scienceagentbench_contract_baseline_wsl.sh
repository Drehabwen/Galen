#!/usr/bin/env bash
set -euo pipefail

ROOT=/opt/galen
IMAGE=sab.eval.x86_64.1:gold
BENCHMARK="$ROOT/evals/public-benchmarks/downloads/benchmark_verified/benchmark"
CONTRACT="$ROOT/evals/contracts/scienceagentbench-clintox-v1.json"
PROGRAM="$ROOT/evals/baselines/scienceagentbench/clintox_ecfp_logreg.py"
SCORER="$ROOT/scripts/evals/score_scienceagentbench_clintox.py"
RUN_DIR="$ROOT/evals/runs/scienceagentbench-clintox-contract-baseline"
RESULT="$RUN_DIR/result.json"
SCORE="$RUN_DIR/score.json"
PROGRAM_SHA=$(sha256sum "$PROGRAM" | cut -d' ' -f1)
CONTRACT_SHA=$(sha256sum "$CONTRACT" | cut -d' ' -f1)
TRAIN_SHA=$(sha256sum "$BENCHMARK/datasets/clintox/clintox_train.csv" | cut -d' ' -f1)
TEST_SHA=$(sha256sum "$BENCHMARK/datasets/clintox/clintox_test.csv" | cut -d' ' -f1)
IMAGE_ID=$(docker image inspect "$IMAGE" --format '{{.Id}}')

mkdir -p "$RUN_DIR/pred_results"
if [[ -f "$RESULT" ]] && /opt/scienceagentbench-harness/bin/python - \
  "$RESULT" "$PROGRAM_SHA" "$CONTRACT_SHA" "$TRAIN_SHA" "$TEST_SHA" "$IMAGE_ID" <<'PY'
import json, sys
value = json.load(open(sys.argv[1], encoding="utf-8"))
expected = {
    "program_sha256": sys.argv[2],
    "contract_sha256": sys.argv[3],
    "train_sha256": sys.argv[4],
    "test_sha256": sys.argv[5],
    "image_id": sys.argv[6],
}
ok = value.get("official_success") == 1 and value.get("provenance") == expected
raise SystemExit(0 if ok else 1)
PY
then
  echo "existing successful baseline retained: $RESULT"
  exit 0
fi

/opt/scienceagentbench-harness/bin/python "$ROOT/scripts/evals/scienceagentbench_contract.py" \
  --contract "$CONTRACT" --benchmark-root "$BENCHMARK" > "$RUN_DIR/preflight.json"
rm -f "$RUN_DIR/pred_results/clintox_test_pred.csv"

docker run --rm --network none \
  -e GALEN_TASK_CONTRACT=/testbed/task-contract.json \
  -v "$BENCHMARK:/testbed/benchmark:ro" \
  -v "$CONTRACT:/testbed/task-contract.json:ro" \
  -v "$PROGRAM:/testbed/program_to_eval/pred_clintox_nn.py:ro" \
  -v "$RUN_DIR/pred_results:/testbed/pred_results" \
  --entrypoint python "$IMAGE" -m program_to_eval.pred_clintox_nn \
  > "$RUN_DIR/program.stdout.log" 2> "$RUN_DIR/program.stderr.log"

docker run --rm --network none \
  -v "$BENCHMARK:/testbed/benchmark:ro" \
  -v "$RUN_DIR/pred_results:/testbed/pred_results:ro" \
  -v "$SCORER:/testbed/score_clintox.py:ro" \
  --entrypoint python "$IMAGE" /testbed/score_clintox.py \
  > "$SCORE"

/opt/scienceagentbench-harness/bin/python - \
  "$SCORE" "$RESULT" "$PROGRAM_SHA" "$CONTRACT_SHA" "$TRAIN_SHA" "$TEST_SHA" "$IMAGE_ID" <<'PY'
import json, sys
score = json.load(open(sys.argv[1], encoding="utf-8"))
score.update({
    "schema_version": 1,
    "baseline_id": "clintox-ecfp-logreg-v1",
    "contract_id": "scienceagentbench-clintox-v1",
    "provenance": {
        "program_sha256": sys.argv[3],
        "contract_sha256": sys.argv[4],
        "train_sha256": sys.argv[5],
        "test_sha256": sys.argv[6],
        "image_id": sys.argv[7],
    },
})
with open(sys.argv[2], "w", encoding="utf-8") as stream:
    json.dump(score, stream, ensure_ascii=False, indent=2)
    stream.write("\n")
PY

cat "$RESULT"
