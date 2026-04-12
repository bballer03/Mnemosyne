#!/usr/bin/env bash

set -euo pipefail

usage() {
    cat >&2 <<'EOF'
usage: ./scripts/run_step11_scaling_validation.sh --output-dir <dir> [--sizes-mb "500 1024 2048"]
EOF
}

run_measure_script() {
    local sanitized_script="$script_dir/.measure_rss.lf.sh"
    tr -d '\r' < "$measure_script" > "$sanitized_script"
    chmod +x "$sanitized_script"
    "$sanitized_script" "$@"
    rm -f "$sanitized_script"
}

read_proc_value() {
    local key=$1
    local file=$2
    awk -v key="$key" '$1 == key":" { print $2; exit }' "$file" 2>/dev/null || true
}

format_human_bytes() {
    local bytes=$1
    awk -v bytes="$bytes" 'BEGIN {
        if (bytes >= 1073741824) printf "%.2f GiB", bytes / 1073741824;
        else if (bytes >= 1048576) printf "%.2f MiB", bytes / 1048576;
        else if (bytes >= 1024) printf "%.2f KiB", bytes / 1024;
        else printf "%d B", bytes;
    }'
}

format_human_kib() {
    local kib=$1
    awk -v kib="$kib" 'BEGIN {
        if (kib >= 1048576) printf "%.2f GiB", kib / 1048576;
        else if (kib >= 1024) printf "%.2f MiB", kib / 1024;
        else printf "%d KiB", kib;
    }'
}

ratio_value_for() {
    local rss_kib=$1
    local dump_bytes=$2
    awk -v rss_kib="$rss_kib" -v dump_bytes="$dump_bytes" 'BEGIN {
        if (dump_bytes <= 0) print 0;
        else printf "%.4f", (rss_kib * 1024) / dump_bytes;
    }'
}

ratio_display_for() {
    local rss_kib=$1
    local dump_bytes=$2
    awk -v rss_kib="$rss_kib" -v dump_bytes="$dump_bytes" 'BEGIN {
        if (dump_bytes <= 0) printf "0.00x";
        else printf "%.2fx", (rss_kib * 1024) / dump_bytes;
    }'
}

status_for_ratio() {
    local ratio=$1
    awk -v ratio="$ratio" 'BEGIN {
        if (ratio <= 4.0) print "✅";
        else if (ratio <= 6.0) print "⚠️";
        else print "❌";
    }'
}

profile_investigation_with_proc() {
    local cli_bin=$1
    local heap=$2
    local pid
    local max_rss_kib=0
    local child_status
    local sample_kib
    local status_path
    local vm_hwm
    local vm_rss

    "$cli_bin" analyze "$heap" --threads --strings --collections >/dev/null 2>&1 &
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
        sleep 0.1
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
        echo "error: unable to determine investigation RSS from /proc for $heap" >&2
        return 1
    fi

    printf '%s\n' "$max_rss_kib"
}

run_investigation_measurement() {
    local heap=$1
    local measurement_file=$2
    local cli_bin="$repo_root/target/debug/mnemosyne-cli"
    local timing_output="$measurement_file.time"
    local dump_bytes
    local max_rss_kib
    local ratio_value
    local ratio_display
    local status_marker
    local measurement_method

    if [[ ! -x "$cli_bin" ]]; then
        echo "error: mnemosyne-cli not found at $cli_bin" >&2
        return 1
    fi

    dump_bytes=$(wc -c <"$heap")

    if [[ -x /usr/bin/time ]]; then
        /usr/bin/time -v "$cli_bin" analyze "$heap" --threads --strings --collections >/dev/null 2>"$timing_output"
        max_rss_kib=$(awk -F: '/Maximum resident set size/ {gsub(/^[[:space:]]+/, "", $2); print $2; exit}' "$timing_output")
        measurement_method="/usr/bin/time -v"
        rm -f "$timing_output"
        if [[ -z "$max_rss_kib" ]]; then
            echo "error: unable to determine investigation RSS for $heap" >&2
            return 1
        fi
    else
        max_rss_kib=$(profile_investigation_with_proc "$cli_bin" "$heap")
        measurement_method="/proc/<pid>/status VmHWM polling"
    fi

    ratio_value=$(ratio_value_for "$max_rss_kib" "$dump_bytes")
    ratio_display=$(ratio_display_for "$max_rss_kib" "$dump_bytes")
    status_marker=$(status_for_ratio "$ratio_value")

    {
        printf 'Measurement method: %s\n\n' "$measurement_method"
        printf 'Heap: %s\n' "$heap"
        printf 'Dump size: %s (%s bytes)\n' "$(format_human_bytes "$dump_bytes")" "$dump_bytes"
        printf '%-22s %-12s %-14s %-10s %s\n' "Command" "Dump Size" "Peak RSS" "Ratio" "Status"
        printf '%-22s %-12s %-14s %-10s %s\n' "----------------------" "------------" "--------------" "----------" "------"
        printf '%-22s %-12s %-14s %-10s %s\n' \
            "analyze-investigation" \
            "$(format_human_bytes "$dump_bytes")" \
            "$(format_human_kib "$max_rss_kib")" \
            "$ratio_display" \
            "$status_marker"
    } >"$measurement_file"
}

extract_ratio() {
    local file=$1
    local command_name=$2
    awk -v command_name="$command_name" '$1 == command_name { print $(NF-1); exit }' "$file"
}

ensure_linux_cli() {
    if [[ -x "$repo_root/target/debug/mnemosyne-cli" ]]; then
        return
    fi

    cargo build -p mnemosyne-cli >/dev/null
}

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd "$script_dir/.." && pwd)
generate_script="$script_dir/generate_synthetic_heap.sh"
measure_script="$script_dir/measure_rss.sh"

output_dir=""
sizes_mb="500 1024 2048"
collecting_sizes=0

while [[ $# -gt 0 ]]; do
    if (( collecting_sizes )); then
        if [[ "$1" == --* ]]; then
            collecting_sizes=0
        else
            sizes_mb+=" $1"
            shift
            continue
        fi
    fi

    case "$1" in
        --output-dir)
            shift
            output_dir=${1:-}
            ;;
        --sizes-mb)
            shift
            sizes_mb=${1:-}
            collecting_sizes=1
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

if [[ -z "$output_dir" ]]; then
    usage
    exit 1
fi

mkdir -p "$output_dir"
summary_file="$output_dir/summary.tsv"
printf 'heap_target_mb\tdump_path\tdump_bytes\tparse_ratio\tanalyze_ratio\tleaks_ratio\tinvestigation_ratio\n' >"$summary_file"

ensure_linux_cli

for size_mb in $sizes_mb; do
    dump_path="$output_dir/synthetic-${size_mb}mb.hprof"
    default_measure="$output_dir/synthetic-${size_mb}mb.default.measure.txt"
    investigation_measure="$output_dir/synthetic-${size_mb}mb.investigation.measure.txt"

    bash "$generate_script" \
        --size-mb "$size_mb" \
        --xmx-mb $(( size_mb + (size_mb / 2) + 256 )) \
        --output "$dump_path"

    run_measure_script --commands "parse analyze leaks" "$dump_path" >"$default_measure"
    run_investigation_measurement "$dump_path" "$investigation_measure"

    dump_bytes=$(wc -c <"$dump_path")
    parse_ratio=$(extract_ratio "$default_measure" parse)
    analyze_ratio=$(extract_ratio "$default_measure" analyze)
    leaks_ratio=$(extract_ratio "$default_measure" leaks)
    investigation_ratio=$(extract_ratio "$investigation_measure" analyze-investigation)

    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
        "$size_mb" \
        "$dump_path" \
        "$dump_bytes" \
        "$parse_ratio" \
        "$analyze_ratio" \
        "$leaks_ratio" \
        "$investigation_ratio" >>"$summary_file"
done

echo "Step 11 scaling validation summary written to $summary_file"
