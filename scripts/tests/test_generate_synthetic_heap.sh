#!/usr/bin/env bash

set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
tmpdir=$(mktemp -d)
output="$tmpdir/synthetic-64mb.hprof"

cleanup() {
    rm -rf "$tmpdir"
}

trap cleanup EXIT

bash "$repo_root/scripts/generate_synthetic_heap.sh" \
    --size-mb 64 \
    --xmx-mb 256 \
    --output "$output"

if [[ ! -s "$output" ]]; then
    echo "expected non-empty heap dump at $output" >&2
    exit 1
fi

dump_bytes=$(wc -c <"$output")
if (( dump_bytes < 1048576 )); then
    echo "expected heap dump >= 1 MiB, got ${dump_bytes} bytes" >&2
    exit 1
fi

echo "generated synthetic heap dump: $output (${dump_bytes} bytes)"
