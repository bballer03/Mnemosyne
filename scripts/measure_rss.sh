#!/usr/bin/env bash

set -euo pipefail

if [[ $# -lt 1 ]]; then
    echo "usage: $0 <heap.hprof> [more heaps...]" >&2
    exit 1
fi

if [[ ! -x /usr/bin/time ]]; then
    echo "error: /usr/bin/time is required for RSS measurement" >&2
    exit 1
fi

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)

resolve_cli() {
    if command -v mnemosyne-cli >/dev/null 2>&1; then
        command -v mnemosyne-cli
        return
    fi
    if [[ -x "$repo_root/target/release/mnemosyne-cli" ]]; then
        echo "$repo_root/target/release/mnemosyne-cli"
        return
    fi
    if [[ -x "$repo_root/target/debug/mnemosyne-cli" ]]; then
        echo "$repo_root/target/debug/mnemosyne-cli"
        return
    fi

    echo "error: mnemosyne-cli not found in PATH or target/{release,debug}" >&2
    exit 1
}

human_kib() {
    local kib=$1
    awk -v kib="$kib" '
        BEGIN {
            if (kib >= 1048576) {
                printf "%.2f GiB", kib / 1048576;
            } else if (kib >= 1024) {
                printf "%.2f MiB", kib / 1024;
            } else {
                printf "%d KiB", kib;
            }
        }
    '
}

cli_bin=$(resolve_cli)

for heap in "$@"; do
    if [[ ! -f "$heap" ]]; then
        echo "error: heap file not found: $heap" >&2
        exit 1
    fi

    echo "Measuring RSS for: $heap"
    timing_output=$(mktemp)
    trap 'rm -f "$timing_output"' EXIT

    /usr/bin/time -v "$cli_bin" parse "$heap" >/dev/null 2>"$timing_output"

    max_rss_kib=$(awk -F: '/Maximum resident set size/ {gsub(/^[[:space:]]+/, "", $2); print $2}' "$timing_output")
    if [[ -z "$max_rss_kib" ]]; then
        echo "  unable to determine maximum resident set size" >&2
        continue
    fi

    echo "  Max RSS: $(human_kib "$max_rss_kib") ($max_rss_kib KiB)"
    rm -f "$timing_output"
    trap - EXIT
done