#!/usr/bin/env bash
set -euo pipefail

VENV=/opt/scienceagentbench-harness

if [[ ! -x "$VENV/bin/python" ]]; then
  apt-get update
  DEBIAN_FRONTEND=noninteractive apt-get install -y python3-venv
  python3 -m venv "$VENV"
fi

"$VENV/bin/pip" install --upgrade pip
"$VENV/bin/pip" install \
  docker==7.1.0 \
  datasets==3.1.0 \
  python-dotenv==1.0.1 \
  tqdm==4.67.0 \
  requests==2.32.3

"$VENV/bin/python" - <<'PY'
import datasets
import docker
import dotenv
import requests
import tqdm

print("SCIENCEAGENTBENCH_WSL_HARNESS_READY")
PY
