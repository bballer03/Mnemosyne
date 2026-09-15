#!/usr/bin/env bash
# Reclaim WSL memory from stuck Bun/jsdom UI test processes.
# Only signals real `bun` binaries running `test` — never parent bash wrappers.
set -euo pipefail

killed=0
while read -r pid cmd; do
  [[ -z "${pid:-}" ]] && continue
  # Match: absolute/relative bun binary + "test" argv (not shell wrappers).
  if [[ "$cmd" == *"/bun test "* ]] || [[ "$cmd" == "bun test "* ]] || [[ "$cmd" == *"/bun test" ]] || [[ "$cmd" == "bun test" ]]; then
    echo "killing pid=$pid cmd=$cmd"
    kill -TERM "$pid" 2>/dev/null || true
    sleep 1
    if kill -0 "$pid" 2>/dev/null; then
      kill -KILL "$pid" 2>/dev/null || true
    fi
    killed=$((killed + 1))
  fi
done < <(ps -eo pid=,args= | grep -E '[/ ]bun test( |$)' || true)

echo "reaped ${killed} stuck ui-test process(es)"
