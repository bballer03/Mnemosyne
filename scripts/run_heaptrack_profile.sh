#!/usr/bin/env bash

set -euo pipefail

DEFAULT_COMMAND="analyze"

usage() {
    cat >&2 <<'EOF'
usage: ./scripts/run_heaptrack_profile.sh [--command "analyze --threads --strings --collections"] [--output <path>] <heap.hprof>

The wrapper always appends <heap.hprof> as the final mnemosyne-cli argument.
Use --command for a subcommand plus optional flags, not for a full arbitrary shell command.
EOF
}

if ! command -v heaptrack >/dev/null 2>&1; then
    echo "skip: heaptrack not installed; profiling wrapper skipped."
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

heap_path=""
command_text=$DEFAULT_COMMAND
output_path=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --command)
            shift
            if [[ $# -eq 0 ]]; then
                echo "error: --command requires a value" >&2
                usage
                exit 1
            fi
            command_text=$1
            ;;
        --output)
            shift
            if [[ $# -eq 0 ]]; then
                echo "error: --output requires a value" >&2
                usage
                exit 1
            fi
            output_path=$1
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

if [[ -z "$heap_path" ]]; then
    usage
    exit 1
fi

if [[ ! -f "$heap_path" ]]; then
    echo "error: heap file not found: $heap_path" >&2
    exit 1
fi

if [[ -z "$output_path" ]]; then
    output_path="$repo_root/heaptrack-$(basename "$heap_path").gz"
fi

cli_bin=$(resolve_cli)
read -r -a cli_args <<<"$command_text"

printf -v command_display '%q ' "$cli_bin" "${cli_args[@]}" "$heap_path"
command_display=${command_display% }

printf 'Running heaptrack profile for %s\n' "$heap_path"
printf 'Heaptrack output: %s\n' "$output_path"
printf 'CLI command: %s\n' "$command_display"

heaptrack -o "$output_path" "$cli_bin" "${cli_args[@]}" "$heap_path"
