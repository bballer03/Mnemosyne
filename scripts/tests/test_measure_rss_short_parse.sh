#!/usr/bin/env bash

set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
tmpdir=$(mktemp -d)

cleanup() {
    rm -rf "$tmpdir"
}

trap cleanup EXIT

heap_path="$tmpdir/synthetic-8mb.hprof"
measure_script_lf="$repo_root/scripts/.measure_rss.test.lf.sh"

bash "$repo_root/scripts/generate_synthetic_heap.sh" \
    --size-mb 8 \
    --xmx-mb 128 \
    --output "$heap_path"

tr -d '\r' < "$repo_root/scripts/measure_rss.sh" > "$measure_script_lf"
chmod +x "$measure_script_lf"

output=$("$measure_script_lf" --commands "parse" "$heap_path" 2>&1)
printf '%s\n' "$output"

rm -f "$measure_script_lf"

if grep -q 'unable to determine peak RSS' <<<"$output"; then
    echo "expected parse RSS measurement without peak RSS failure" >&2
    exit 1
fi

if ! grep -Eq '^parse[[:space:]].*[0-9]+\.[0-9]+x[[:space:]]' <<<"$output"; then
    echo "expected parse measurement row with ratio value" >&2
    exit 1
fi

echo "measure_rss short-parse smoke test passed"
