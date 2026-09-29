#!/usr/bin/env bash
set -euo pipefail

candidate=${1:?usage: run_scienceagentbench_candidate_wsl.sh NAME PROGRAM [RUN_LABEL] [CONTRACT_NAME]}
program=${2:?usage: run_scienceagentbench_candidate_wsl.sh NAME PROGRAM [RUN_LABEL] [CONTRACT_NAME]}
run_label=${3:-scienceagentbench-contract-competition}
contract_name=${4:-scienceagentbench-clintox-v3.json}
if [[ ! "$candidate" =~ ^[a-z0-9][a-z0-9_-]*$ ]]; then
  echo "invalid candidate name: $candidate" >&2
  exit 2
fi
if [[ ! "$run_label" =~ ^[a-z0-9][a-z0-9_-]*$ ]]; then
  echo "invalid run label: $run_label" >&2
  exit 2
fi
if [[ ! "$contract_name" =~ ^scienceagentbench-clintox-v[0-9]+\.json$ ]]; then
  echo "invalid contract name: $contract_name" >&2
  exit 2
fi

ROOT=/opt/galen
IMAGE=sab.eval.x86_64.1:gold
BENCHMARK="$ROOT/evals/public-benchmarks/downloads/benchmark_verified/benchmark"
CONTRACT="$ROOT/evals/contracts/$contract_name"
SCORER="$ROOT/scripts/evals/score_scienceagentbench_clintox.py"
RUN_DIR="$ROOT/evals/runs/$run_label/scores/$candidate"
RESULT="$RUN_DIR/result.json"
SCORE="$RUN_DIR/score.json"

if [[ ! -f "$program" ]]; then
  echo "candidate program missing: $program" >&2
  exit 2
fi
mkdir -p "$RUN_DIR/pred_results"
PROGRAM_SHA=$(sha256sum "$program" | cut -d' ' -f1)
CONTRACT_SHA=$(sha256sum "$CONTRACT" | cut -d' ' -f1)
TRAIN_SHA=$(sha256sum "$BENCHMARK/datasets/clintox/clintox_train.csv" | cut -d' ' -f1)
TEST_SHA=$(sha256sum "$BENCHMARK/datasets/clintox/clintox_test.csv" | cut -d' ' -f1)
IMAGE_ID=$(docker image inspect "$IMAGE" --format '{{.Id}}')

if [[ -f "$RESULT" ]] && /opt/scienceagentbench-harness/bin/python - \
  "$RESULT" "$PROGRAM_SHA" "$CONTRACT_SHA" "$TRAIN_SHA" "$TEST_SHA" "$IMAGE_ID" <<'PY'
import json, sys
value = json.load(open(sys.argv[1], encoding="utf-8"))
expected = {
    "program_sha256": sys.argv[2], "contract_sha256": sys.argv[3],
    "train_sha256": sys.argv[4], "test_sha256": sys.argv[5], "image_id": sys.argv[6],
}
raise SystemExit(0 if value.get("provenance") == expected else 1)
PY
then
  echo "existing candidate result retained: $RESULT"
  cat "$RESULT"
  exit 0
fi

rm -f "$RUN_DIR/pred_results/clintox_test_pred.csv" "$SCORE"
set +e
timeout 1800 docker run --rm --network none \
  -e GALEN_TASK_CONTRACT=/testbed/task-contract.json \
  -v "$BENCHMARK:/testbed/benchmark:ro" \
  -v "$CONTRACT:/testbed/task-contract.json:ro" \
  -v "$program:/testbed/program_to_eval/pred_clintox_nn.py:ro" \
  -v "$RUN_DIR/pred_results:/testbed/pred_results" \
  --entrypoint python "$IMAGE" -m program_to_eval.pred_clintox_nn \
  > "$RUN_DIR/program.stdout.log" 2> "$RUN_DIR/program.stderr.log"
PROGRAM_RC=$?
set -e

if [[ $PROGRAM_RC -eq 0 && -f "$RUN_DIR/pred_results/clintox_test_pred.csv" ]]; then
  set +e
  docker run --rm --network none \
    -v "$BENCHMARK:/testbed/benchmark:ro" \
    -v "$RUN_DIR/pred_results:/testbed/pred_results:ro" \
    -v "$SCORER:/testbed/score_clintox.py:ro" \
    --entrypoint python "$IMAGE" /testbed/score_clintox.py > "$SCORE" 2> "$RUN_DIR/score.stderr.log"
  SCORE_RC=$?
  set -e
else
  SCORE_RC=1
fi

/opt/scienceagentbench-harness/bin/python - \
  "$candidate" "$PROGRAM_RC" "$SCORE_RC" "$SCORE" "$RESULT" \
  "$PROGRAM_SHA" "$CONTRACT_SHA" "$TRAIN_SHA" "$TEST_SHA" "$IMAGE_ID" <<'PY'
import json, pathlib, sys
candidate, program_rc, score_rc = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
score_path, result_path = pathlib.Path(sys.argv[4]), pathlib.Path(sys.argv[5])
if score_path.is_file():
    try:
        result = json.loads(score_path.read_text(encoding="utf-8"))
        result["status"] = "complete"
    except (OSError, json.JSONDecodeError):
        result = None
else:
    result = None
if result is None:
    result = {
        "valid_program": 0,
        "roc_auc": None,
        "threshold": 0.77,
        "threshold_passed": False,
        "official_success": 0,
        "status": "program_failed" if program_rc else "scoring_failed",
    }
result.update({
    "schema_version": 1,
    "candidate": candidate,
    "program_returncode": program_rc,
    "score_returncode": score_rc,
    "provenance": {
        "program_sha256": sys.argv[6], "contract_sha256": sys.argv[7],
        "train_sha256": sys.argv[8], "test_sha256": sys.argv[9], "image_id": sys.argv[10],
    },
})
result_path.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
print(json.dumps(result, ensure_ascii=False, indent=2))
PY
