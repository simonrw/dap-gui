#!/usr/bin/env bash

set -euo pipefail

while true; do
    python -m debugpy.adapter --host 127.0.0.1 --port 5678 --log-stderr
done
