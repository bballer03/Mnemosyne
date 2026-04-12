#!/usr/bin/env bash

set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
tmpdir=$(mktemp -d)

cleanup() {
    rm -rf "$tmpdir"
}

trap cleanup EXIT

output=$(bash "$repo_root/scripts/run_step11_scaling_validation.sh" \
    --output-dir "$tmpdir" \
    --sizes-mb "32" 2>&1)

printf '%s\n' "$output"

if grep -q 'awk: fatal' <<<"$output"; then
    echo "expected Step 11 wrapper output without awk fatal errors" >&2
    exit 1
fi

for path in \
    "$tmpdir/synthetic-32mb.hprof" \
    "$tmpdir/synthetic-32mb.default.measure.txt" \
    "$tmpdir/synthetic-32mb.investigation.measure.txt" \
    "$tmpdir/summary.tsv"; do
    if [[ ! -s "$path" ]]; then
        echo "expected non-empty file: $path" >&2
        exit 1
    fi
done

if ! grep -q '^analyze-investigation ' "$tmpdir/synthetic-32mb.investigation.measure.txt"; then
    echo "expected dedicated investigation measurement row" >&2
    exit 1
fi

if ! grep -q $'^heap_target_mb\tdump_path\tdump_bytes\tparse_ratio\tanalyze_ratio\tleaks_ratio\tinvestigation_ratio$' "$tmpdir/summary.tsv"; then
    echo "expected summary header in $tmpdir/summary.tsv" >&2
    exit 1
fi

if ! grep -q $'^32\t' "$tmpdir/summary.tsv"; then
    echo "expected summary row for 32 MiB target heap" >&2
    exit 1
fi

if ! awk -F'\t' 'NR == 2 { exit !($4 ~ /x$/ && $5 ~ /x$/ && $6 ~ /x$/ && $7 ~ /x$/) }' "$tmpdir/summary.tsv"; then
    echo "expected ratio columns in summary.tsv to use x-suffixed values" >&2
    exit 1
fi

echo "step11 scaling validation smoke test passed"
