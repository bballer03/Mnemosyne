#!/usr/bin/env bash

set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
tmpdir=$(mktemp -d)
output="$tmpdir/synthetic-32mb.hprof"

cleanup() {
    rm -rf "$tmpdir"
}

trap cleanup EXIT

bash "$repo_root/scripts/generate_synthetic_heap.sh" \
    --size-mb 32 \
    --xmx-mb 304 \
    --output "$output"

analyze_output=$(bash -lc '"$PWD/target/debug/mnemosyne-cli" analyze "$1" --format toon' -- "$output")
printf '%s\n' "$analyze_output"

total_instances=$(awk -F= '/^[[:space:]]*total_instances=/ { print $2; exit }' <<<"$analyze_output")
if [[ -z "$total_instances" ]]; then
    echo "expected analyze output to include total_instances" >&2
    exit 1
fi

if (( total_instances < 100000 )); then
    echo "expected at least 100000 total instances in 32MB synthetic heap, got $total_instances" >&2
    exit 1
fi

echo "synthetic heap density smoke test passed with $total_instances total instances"
