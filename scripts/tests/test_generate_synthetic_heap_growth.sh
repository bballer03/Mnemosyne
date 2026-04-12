#!/usr/bin/env bash

set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
tmpdir=$(mktemp -d)
small_heap="$tmpdir/synthetic-16mb.hprof"
large_heap="$tmpdir/synthetic-32mb.hprof"

cleanup() {
    rm -rf "$tmpdir"
}

trap cleanup EXIT

bash "$repo_root/scripts/generate_synthetic_heap.sh" \
    --size-mb 16 \
    --xmx-mb 256 \
    --output "$small_heap"

bash "$repo_root/scripts/generate_synthetic_heap.sh" \
    --size-mb 32 \
    --xmx-mb 320 \
    --output "$large_heap"

small_analyze=$(bash -lc '"$PWD/target/debug/mnemosyne-cli" analyze "$1" --format toon' -- "$small_heap")
large_analyze=$(bash -lc '"$PWD/target/debug/mnemosyne-cli" analyze "$1" --format toon' -- "$large_heap")

printf 'Small analyze output:\n%s\n' "$small_analyze"
printf 'Large analyze output:\n%s\n' "$large_analyze"

small_instances=$(awk -F= '/^[[:space:]]*total_instances=/ { print $2; exit }' <<<"$small_analyze")
large_instances=$(awk -F= '/^[[:space:]]*total_instances=/ { print $2; exit }' <<<"$large_analyze")

if [[ -z "$small_instances" || -z "$large_instances" ]]; then
    echo "expected analyze output to include total_instances for both heaps" >&2
    exit 1
fi

if (( large_instances < small_instances + 20000 )); then
    echo "expected larger synthetic heap to produce materially more total instances, got small=$small_instances large=$large_instances" >&2
    exit 1
fi

echo "synthetic heap growth smoke test passed: small=$small_instances large=$large_instances"
