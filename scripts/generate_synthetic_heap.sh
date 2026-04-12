#!/usr/bin/env bash

set -euo pipefail

usage() {
    cat >&2 <<'EOF'
usage: ./scripts/generate_synthetic_heap.sh --size-mb <mb> --output <file.hprof> [--xmx-mb <mb>]
EOF
}

resolve_tool() {
    local tool=$1
    if command -v "$tool" >/dev/null 2>&1; then
        command -v "$tool"
        return
    fi

    for candidate in \
        "/usr/lib/jvm/java-17-amazon-corretto/bin/$tool" \
        "/usr/lib/jvm/default-java/bin/$tool"; do
        if [[ -x "$candidate" ]]; then
            printf '%s\n' "$candidate"
            return
        fi
    done

    return 1
}

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
source_file="$script_dir/java/SyntheticHeapApp.java"

size_mb=""
output=""
xmx_mb=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --size-mb)
            shift
            size_mb=${1:-}
            ;;
        --output)
            shift
            output=${1:-}
            ;;
        --xmx-mb)
            shift
            xmx_mb=${1:-}
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "error: unknown option '$1'" >&2
            usage
            exit 1
            ;;
    esac
    shift
done

if [[ -z "$size_mb" || -z "$output" ]]; then
    usage
    exit 1
fi

if ! [[ "$size_mb" =~ ^[0-9]+$ ]] || (( size_mb <= 0 )); then
    echo "error: --size-mb must be a positive integer" >&2
    exit 1
fi

if [[ -z "$xmx_mb" ]]; then
    xmx_mb=$(( size_mb + (size_mb / 2) + 256 ))
fi

if ! [[ "$xmx_mb" =~ ^[0-9]+$ ]] || (( xmx_mb <= size_mb )); then
    echo "error: --xmx-mb must be a positive integer larger than --size-mb" >&2
    exit 1
fi

if ! javac_bin=$(resolve_tool javac); then
    echo "error: required tool 'javac' not found in PATH" >&2
    exit 1
fi

if ! java_bin=$(resolve_tool java); then
    echo "error: required tool 'java' not found in PATH" >&2
    exit 1
fi

if ! jmap_bin=$(resolve_tool jmap); then
    echo "error: required tool 'jmap' not found in PATH" >&2
    exit 1
fi

for tool_path in "$javac_bin" "$java_bin" "$jmap_bin"; do
    if [[ ! -x "$tool_path" ]]; then
        echo "error: required tool '$tool' not found in PATH" >&2
        exit 1
    fi
done

if [[ ! -f "$source_file" ]]; then
    echo "error: source file not found: $source_file" >&2
    exit 1
fi

mkdir -p "$(dirname "$output")"
rm -f "$output"

build_dir=$(mktemp -d)
log_file="$build_dir/synthetic-heap.log"
app_pid=""

cleanup() {
    if [[ -n "$app_pid" ]] && kill -0 "$app_pid" >/dev/null 2>&1; then
        kill "$app_pid" >/dev/null 2>&1 || true
        wait "$app_pid" >/dev/null 2>&1 || true
    fi
    rm -rf "$build_dir"
}

trap cleanup EXIT

"$javac_bin" -d "$build_dir" "$source_file"

"$java_bin" -Xmx"${xmx_mb}m" -cp "$build_dir" SyntheticHeapApp "$size_mb" >"$log_file" 2>&1 &
app_pid=$!

ready=0
for _ in $(seq 1 120); do
    if ! kill -0 "$app_pid" >/dev/null 2>&1; then
        cat "$log_file" >&2
        echo "error: synthetic heap app exited before becoming ready" >&2
        exit 1
    fi
    if grep -q '^READY ' "$log_file"; then
        ready=1
        break
    fi
    sleep 1
done

if (( ready == 0 )); then
    cat "$log_file" >&2
    echo "error: synthetic heap app did not become ready within 120 seconds" >&2
    exit 1
fi

"$jmap_bin" -dump:format=b,file="$output" "$app_pid" >/dev/null

if [[ ! -s "$output" ]]; then
    echo "error: heap dump was not created: $output" >&2
    exit 1
fi

dump_bytes=$(wc -c <"$output")
echo "Synthetic heap dump written to $output (${dump_bytes} bytes)"
