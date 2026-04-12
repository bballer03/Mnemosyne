#!/usr/bin/env bash

set -euo pipefail

DEFAULT_COMMANDS="parse analyze leaks"
SAMPLE_INTERVAL_SECONDS="0.1"

usage() {
    cat >&2 <<'EOF'
usage: ./scripts/measure_rss.sh [--commands "parse analyze leaks"] <heap.hprof> [more heaps...]
EOF
}

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

human_bytes() {
    local bytes=$1
    awk -v bytes="$bytes" '
        BEGIN {
            if (bytes >= 1073741824) {
                printf "%.2f GiB", bytes / 1073741824;
            } else if (bytes >= 1048576) {
                printf "%.2f MiB", bytes / 1048576;
            } else if (bytes >= 1024) {
                printf "%.2f KiB", bytes / 1024;
            } else {
                printf "%d B", bytes;
            }
        }
    '
}

ratio_for() {
    local rss_kib=$1
    local dump_bytes=$2
    awk -v rss_kib="$rss_kib" -v dump_bytes="$dump_bytes" '
        BEGIN {
            if (dump_bytes <= 0) {
                printf "0.00x";
            } else {
                printf "%.2fx", (rss_kib * 1024) / dump_bytes;
            }
        }
    '
}

status_for_ratio() {
    local ratio=$1
    awk -v ratio="$ratio" '
        BEGIN {
            if (ratio <= 4.0) {
                print "✅";
            } else if (ratio <= 6.0) {
                print "⚠️";
            } else {
                print "❌";
            }
        }
    '
}

profile_with_time() {
    local cli_bin=$1
    local command_name=$2
    local heap=$3
    local timing_output
    local max_rss_kib

    timing_output=$(mktemp)
    if /usr/bin/time -v "$cli_bin" "$command_name" "$heap" >/dev/null 2>"$timing_output"; then
        :
    else
        local status=$?
        cat "$timing_output" >&2
        rm -f "$timing_output"
        return "$status"
    fi

    max_rss_kib=$(awk -F: '/Maximum resident set size/ {gsub(/^[[:space:]]+/, "", $2); print $2}' "$timing_output")
    rm -f "$timing_output"

    if [[ -z "$max_rss_kib" ]]; then
        echo "error: unable to determine maximum resident set size for command '$command_name'" >&2
        return 1
    fi

    printf '%s\n' "$max_rss_kib"
}

profile_with_proc() {
    local cli_bin=$1
    local command_name=$2
    local heap=$3
    local pid
    local max_rss_kib=0
    local child_status
    local sample_kib
    local status_path
    local vm_hwm
    local vm_rss

    read_proc_value() {
        local key=$1
        local file=$2
        awk -v key="$key" '$1 == key":" { print $2; exit }' "$file" 2>/dev/null || true
    }

    "$cli_bin" "$command_name" "$heap" >/dev/null 2>&1 &
    pid=$!
    status_path="/proc/$pid/status"

    while kill -0 "$pid" >/dev/null 2>&1; do
        if [[ -r "$status_path" ]]; then
            vm_hwm=$(read_proc_value VmHWM "$status_path")
            vm_rss=$(read_proc_value VmRSS "$status_path")
            sample_kib=${vm_hwm:-${vm_rss:-0}}
            if [[ -n "$sample_kib" ]] && (( sample_kib > max_rss_kib )); then
                max_rss_kib=$sample_kib
            fi
        fi
        sleep "$SAMPLE_INTERVAL_SECONDS"
    done

    if [[ -r "$status_path" ]]; then
        vm_hwm=$(read_proc_value VmHWM "$status_path")
        vm_rss=$(read_proc_value VmRSS "$status_path")
        sample_kib=${vm_hwm:-${vm_rss:-0}}
        if [[ -n "$sample_kib" ]] && (( sample_kib > max_rss_kib )); then
            max_rss_kib=$sample_kib
        fi
    fi

    wait "$pid"
    child_status=$?
    if (( child_status != 0 )); then
        return "$child_status"
    fi

    if (( max_rss_kib <= 0 )); then
        echo "error: unable to determine peak RSS from /proc for command '$command_name'" >&2
        return 1
    fi

    printf '%s\n' "$max_rss_kib"
}

measure_command() {
    local cli_bin=$1
    local command_name=$2
    local heap=$3

    if [[ -x /usr/bin/time ]]; then
        profile_with_time "$cli_bin" "$command_name" "$heap"
    else
        profile_with_proc "$cli_bin" "$command_name" "$heap"
    fi
}

commands=()
heaps=()

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
        --help|-h)
            usage
            exit 0
            ;;
        --*)
            echo "error: unknown option: $1" >&2
            usage
            exit 1
            ;;
        *)
            heaps+=("$1")
            ;;
    esac
    shift
done

if [[ ${#commands[@]} -eq 0 ]]; then
    read -r -a commands <<<"$DEFAULT_COMMANDS"
fi

if [[ ${#heaps[@]} -eq 0 ]]; then
    usage
    exit 1
fi

cli_bin=$(resolve_cli)

if [[ -x /usr/bin/time ]]; then
    measurement_method="/usr/bin/time -v"
else
    measurement_method="/proc/<pid>/status VmHWM polling"
fi

printf 'Measurement method: %s\n' "$measurement_method"

for heap in "${heaps[@]}"; do
    if [[ ! -f "$heap" ]]; then
        echo "error: heap file not found: $heap" >&2
        exit 1
    fi
    local_dump_bytes=$(wc -c <"$heap")
    if [[ -z "$local_dump_bytes" ]]; then
        echo "error: unable to determine file size for $heap" >&2
        exit 1
    fi

    printf '\nHeap: %s\n' "$heap"
    printf 'Dump size: %s (%s bytes)\n' "$(human_bytes "$local_dump_bytes")" "$local_dump_bytes"
    printf '%-10s %-12s %-14s %-10s %s\n' "Command" "Dump Size" "Peak RSS" "Ratio" "Status"
    printf '%-10s %-12s %-14s %-10s %s\n' "----------" "------------" "--------------" "----------" "------"

    for command_name in "${commands[@]}"; do
        max_rss_kib=$(measure_command "$cli_bin" "$command_name" "$heap")
        ratio_value=$(awk -v rss_kib="$max_rss_kib" -v dump_bytes="$local_dump_bytes" 'BEGIN { if (dump_bytes <= 0) print 0; else printf "%.4f", (rss_kib * 1024) / dump_bytes }')
        ratio_display=$(ratio_for "$max_rss_kib" "$local_dump_bytes")
        status_marker=$(status_for_ratio "$ratio_value")
        printf '%-10s %-12s %-14s %-10s %s\n' \
            "$command_name" \
            "$(human_bytes "$local_dump_bytes")" \
            "$(human_kib "$max_rss_kib")" \
            "$ratio_display" \
            "$status_marker"
    done
done
