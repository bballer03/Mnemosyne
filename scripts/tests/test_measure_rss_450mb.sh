#!/usr/bin/env bash

set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
tmpdir=$(mktemp -d)

cleanup() {
    rm -rf "$tmpdir"
    rm -f "$repo_root/scripts/.measure_rss.test.lf.sh"
}

trap cleanup EXIT

heap_path="$tmpdir/synthetic-450mb.hprof"
measure_script_lf="$repo_root/scripts/.measure_rss.test.lf.sh"

bash "$repo_root/scripts/generate_synthetic_heap.sh" \
    --size-mb 450 \
    --xmx-mb 931 \
    --output "$heap_path"

tr -d '\r' < "$repo_root/scripts/measure_rss.sh" > "$measure_script_lf"
chmod +x "$measure_script_lf"

output=$("$measure_script_lf" --commands "parse analyze leaks" "$heap_path" 2>&1)
printf '%s\n' "$output"

for command_name in parse analyze leaks; do
    if ! grep -Eq "^${command_name}[[:space:]].*[0-9]+\.[0-9]+x[[:space:]]" <<<"$output"; then
        echo "expected ${command_name} measurement row with ratio value" >&2
        exit 1
    fi
done

echo "measure_rss 450mb smoke test passed"
