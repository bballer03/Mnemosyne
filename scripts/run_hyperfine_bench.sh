#!/usr/bin/env bash

set -euo pipefail

DEFAULT_COMMANDS="parse analyze leaks"
DEFAULT_RUNS=3
DEFAULT_WARMUP=1

usage() {
    cat >&2 <<'EOF'
usage: ./scripts/run_hyperfine_bench.sh [--commands "parse analyze leaks"] [--runs 3] [--warmup 1] <heap.hprof>
EOF
}

if ! command -v hyperfine >/dev/null 2>&1; then
    echo "skip: hyperfine not installed; benchmark wrapper skipped."
    exit 0
fi

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)

resolve_cli() {
    if command -v mnemosyne-cli >/dev/null 2>&1; then
        command -v mnemosyne-cli
        return
    fi
    if [[ -x "$repo_root/target/release/mnemosyne-cli" ]]; then
        printf '%s\n' "$repo_root/target/release/mnemosyne-cli"
        return
    fi
    if [[ -x "$repo_root/target/debug/mnemosyne-cli" ]]; then
        printf '%s\n' "$repo_root/target/debug/mnemosyne-cli"
        return
    fi

    echo "error: mnemosyne-cli not found in PATH or target/{release,debug}" >&2
    exit 1
}

commands=()
runs=$DEFAULT_RUNS
warmup=$DEFAULT_WARMUP
heap_path=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --commands)
            shift
            if [[ $# -eq 0 ]]; then
                echo "error: --commands requires a value" >&2
                usage
                exit 1
            fi
            read -r -a commands <<<"$1"
            ;;
        --runs)
            shift
            if [[ $# -eq 0 ]]; then
                echo "error: --runs requires a value" >&2
                usage
                exit 1
            fi
            runs=$1
            ;;
        --warmup)
            shift
            if [[ $# -eq 0 ]]; then
                echo "error: --warmup requires a value" >&2
                usage
                exit 1
            fi
            warmup=$1
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        --*)
            echo "error: unknown option: $1" >&2
            usage
            exit 1
            ;;
        *)
            if [[ -n "$heap_path" ]]; then
                echo "error: only one heap path is supported" >&2
                usage
                exit 1
            fi
            heap_path=$1
            ;;
    esac
    shift
done

if [[ ${#commands[@]} -eq 0 ]]; then
    read -r -a commands <<<"$DEFAULT_COMMANDS"
fi

if [[ -z "$heap_path" ]]; then
    usage
    exit 1
fi

if [[ ! -f "$heap_path" ]]; then
    echo "error: heap file not found: $heap_path" >&2
    exit 1
fi

cli_bin=$(resolve_cli)
printf -v escaped_cli '%q' "$cli_bin"
printf -v escaped_heap '%q' "$heap_path"

hyperfine_args=(--warmup "$warmup" --runs "$runs")

for command_name in "${commands[@]}"; do
    hyperfine_args+=(--command-name "$command_name" "$escaped_cli $command_name $escaped_heap")
done

printf 'Running hyperfine benchmark matrix for %s\n' "$heap_path"
hyperfine "${hyperfine_args[@]}"
