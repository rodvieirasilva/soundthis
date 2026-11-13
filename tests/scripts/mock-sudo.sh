#!/usr/bin/env bash
# Mock sudo for running unprivileged commands in tests
set -euo pipefail

while [[ "${1:-}" == -* ]]; do
  if [[ "$1" == "-u" ]]; then
    shift 2
  else
    shift 1
  fi
done

exec "$@"
