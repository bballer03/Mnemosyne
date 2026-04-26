#!/usr/bin/env bash

set -euo pipefail

DEFAULT_RUNS=5
DEFAULT_TOOLS="mnemo-deep,mnemo-overview,mat,hprof-slurp"
DEFAULT_FIXTURES="small,medium,large,xlarge"

bench_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd "$bench_dir/../.." && pwd)
measure_script="$bench_dir/measure_run.sh"

usage() {
    cat >&2 <<'EOF'
usage: scripts/bench/run_comparative.sh --fixtures-dir <dir> --output-dir <dir> [--runs <n>] [--tools <csv>] [--fixtures <csv>]

Tool names:
  mnemo-deep,mnemo-overview,mat,hprof-slurp

Fixture labels:
  small,medium,large,xlarge

Environment:
  MNEMOSYNE_BIN  Optional explicit path to mnemosyne-cli.
  MAT_HOME       Required when tool set includes mat.
  MAT_VMARGS     Optional JVM heap setting for MAT. Defaults to -Xmx16g.

Linux is the published reference path. Windows users should run this harness via WSL2.
EOF
}

fail() {
    printf 'error: %s\n' "$1" >&2
    exit 1
}

warn() {
    printf 'warning: %s\n' "$1" >&2
}

trim() {
    local value=$1
    value=${value#"${value%%[![:space:]]*}"}
    value=${value%"${value##*[![:space:]]}"}
    printf '%s' "$value"
}

split_csv() {
    local value=$1
    local -n out_ref=$2
    local raw=()
    local item=""

    IFS=',' read -r -a raw <<<"$value"
    out_ref=()
    for item in "${raw[@]}"; do
        item=$(trim "$item")
        if [[ -n "$item" ]]; then
            out_ref+=("$item")
        fi
    done
}

resolve_python() {
    if command -v python3 >/dev/null 2>&1; then
        command -v python3
        return 0
    fi

    if command -v python >/dev/null 2>&1; then
        command -v python
        return 0
    fi

    return 1
}

resolve_mnemosyne_bin() {
    if [[ -n "${MNEMOSYNE_BIN:-}" ]]; then
        [[ -x "$MNEMOSYNE_BIN" ]] || fail "MNEMOSYNE_BIN is set but not executable: $MNEMOSYNE_BIN"
        printf '%s\n' "$MNEMOSYNE_BIN"
        return 0
    fi

    if command -v mnemosyne >/dev/null 2>&1; then
        command -v mnemosyne
        return 0
    fi

    if command -v mnemosyne-cli >/dev/null 2>&1; then
        command -v mnemosyne-cli
        return 0
    fi

    if [[ -x "$repo_root/target/release/mnemosyne-cli" ]]; then
        printf '%s\n' "$repo_root/target/release/mnemosyne-cli"
        return 0
    fi

    if [[ -x "$repo_root/target/debug/mnemosyne-cli" ]]; then
        printf '%s\n' "$repo_root/target/debug/mnemosyne-cli"
        return 0
    fi

    return 1
}

resolve_mat_bin() {
    [[ -n "${MAT_HOME:-}" ]] || return 1
    [[ -x "$MAT_HOME/ParseHeapDump.sh" ]] || return 1
    printf '%s\n' "$MAT_HOME/ParseHeapDump.sh"
}

resolve_hprof_slurp_bin() {
    command -v hprof-slurp >/dev/null 2>&1 || return 1
    command -v hprof-slurp
}

fixture_path_for_label() {
    local fixture_label=$1
    local candidates=()
    local candidate=""

    case "$fixture_label" in
        small)
            candidates=(
                "$fixtures_dir/heap.hprof"
                "$repo_root/resources/test-fixtures/heap.hprof"
            )
            ;;
        medium)
            candidates=(
                "$fixtures_dir/synthetic-1gb.hprof"
                "$fixtures_dir/synthetic-1024mb.hprof"
            )
            ;;
        large)
            candidates=(
                "$fixtures_dir/synthetic-4gb.hprof"
                "$fixtures_dir/synthetic-4096mb.hprof"
            )
            ;;
        xlarge)
            candidates=(
                "$fixtures_dir/synthetic-10gb.hprof"
                "$fixtures_dir/synthetic-10240mb.hprof"
            )
            ;;
        *)
            fail "unsupported fixture label: $fixture_label"
            ;;
    esac

    for candidate in "${candidates[@]}"; do
        if [[ -f "$candidate" ]]; then
            printf '%s\n' "$candidate"
            return 0
        fi
    done

    fail "fixture '$fixture_label' not found. Looked for: ${candidates[*]}"
}

cleanup_mat_artifacts() {
    local fixture_path=$1

    rm -rf -- \
        "${fixture_path}_Indexes" \
        "${fixture_path}_Suspects.zip" \
        "${fixture_path}_System_Overview.zip" \
        "${fixture_path}_Leak_Suspects.zip" \
        "${fixture_path}_Overview.zip"
}

append_csv_row() {
    local fixture_label=$1
    local run_index=$2
    local json_line=$3

    CSV_PATH="$results_csv" FIXTURE_LABEL="$fixture_label" RUN_INDEX="$run_index" JSON_LINE="$json_line" "$python_bin" - <<'PY'
import csv
import json
import os
from pathlib import Path

row = json.loads(os.environ["JSON_LINE"])
csv_path = Path(os.environ["CSV_PATH"])

with csv_path.open("a", newline="", encoding="utf-8") as handle:
    writer = csv.writer(handle)
    writer.writerow(
        [
            os.environ["FIXTURE_LABEL"],
            row.get("fixture_size_bytes", ""),
            row.get("tool", ""),
            os.environ["RUN_INDEX"],
            row.get("wall_time_seconds", ""),
            row.get("max_rss_kb", ""),
            row.get("exit_code", ""),
            row.get("status", ""),
            row.get("timestamp_utc", ""),
        ]
    )
PY
}

print_summary_table() {
    FIXTURE_ORDER="$(IFS=,; printf '%s' "${selected_fixtures[*]}")" \
    TOOL_ORDER="$(IFS=,; printf '%s' "${available_tools[*]}")" \
    "$python_bin" - "$results_csv" <<'PY'
import csv
import os
import statistics
import sys
from collections import defaultdict

results_csv = sys.argv[1]
fixture_order = {name: index for index, name in enumerate(filter(None, os.environ.get("FIXTURE_ORDER", "").split(",")))}
tool_order = {name: index for index, name in enumerate(filter(None, os.environ.get("TOOL_ORDER", "").split(",")))}

groups = defaultdict(list)
with open(results_csv, newline="", encoding="utf-8") as handle:
    reader = csv.DictReader(handle)
    rows = list(reader)

if not rows:
    print("No result rows recorded.")
    sys.exit(0)

for row in rows:
    groups[(row["fixture"], row["tool"])].append(row)

summary_rows = []
for (fixture, tool), items in groups.items():
    wall_times = [float(item["wall_time_seconds"]) for item in items if item["wall_time_seconds"]]
    rss_values = [int(float(item["max_rss_kb"])) for item in items if item["max_rss_kb"]]
    success_count = sum(1 for item in items if item["status"] == "ok" and int(item["exit_code"]) == 0)
    summary_rows.append(
        {
            "fixture": fixture,
            "tool": tool,
            "median_wall": statistics.median(wall_times) if wall_times else float("nan"),
            "median_rss": statistics.median(rss_values) if rss_values else float("nan"),
            "success_rate": f"{success_count}/{len(items)}",
        }
    )

summary_rows.sort(key=lambda row: (fixture_order.get(row["fixture"], 999), tool_order.get(row["tool"], 999), row["fixture"], row["tool"]))

headers = ["fixture", "tool", "median_wall_s", "median_rss_kb", "success_rate"]
table_rows = []
for row in summary_rows:
    table_rows.append(
        [
            row["fixture"],
            row["tool"],
            f"{row['median_wall']:.6f}",
            str(int(row["median_rss"])),
            row["success_rate"],
        ]
    )

widths = [len(header) for header in headers]
for row in table_rows:
    for index, cell in enumerate(row):
        widths[index] = max(widths[index], len(cell))

def render_line(values):
    return "  ".join(value.ljust(widths[index]) for index, value in enumerate(values))

print(render_line(headers))
print(render_line(["-" * width for width in widths]))
for row in table_rows:
    print(render_line(row))
PY
}

declare -a TOOL_COMMAND=()
TOOL_TIMEOUT_SECONDS=0

prepare_tool_command() {
    local tool_name=$1
    local fixture_path=$2

    TOOL_COMMAND=()
    TOOL_TIMEOUT_SECONDS=0

    case "$tool_name" in
        mnemo-deep)
            TOOL_TIMEOUT_SECONDS=1800
            TOOL_COMMAND=("$mnemosyne_bin" "analyze" "$fixture_path" "--mode" "deep" "--format" "json")
            ;;
        mnemo-overview)
            TOOL_TIMEOUT_SECONDS=600
            TOOL_COMMAND=("$mnemosyne_bin" "analyze" "$fixture_path" "--mode" "overview" "--format" "json" "--top-n" "100")
            ;;
        mat)
            TOOL_TIMEOUT_SECONDS=3600
            TOOL_COMMAND=("$mat_bin" "$fixture_path" "org.eclipse.mat.api:suspects" "org.eclipse.mat.api:overview")
            ;;
        hprof-slurp)
            TOOL_TIMEOUT_SECONDS=600
            TOOL_COMMAND=("$hprof_slurp_bin" "-i" "$fixture_path")
            ;;
        *)
            fail "unsupported tool name: $tool_name"
            ;;
    esac
}

fixtures_dir=""
output_dir=""
runs=$DEFAULT_RUNS
tools_csv=$DEFAULT_TOOLS
fixtures_csv=$DEFAULT_FIXTURES

while [[ $# -gt 0 ]]; do
    case "$1" in
        --fixtures-dir)
            shift
            fixtures_dir=${1:-}
            ;;
        --output-dir)
            shift
            output_dir=${1:-}
            ;;
        --runs)
            shift
            runs=${1:-}
            ;;
        --tools)
            shift
            tools_csv=${1:-}
            ;;
        --fixtures)
            shift
            fixtures_csv=${1:-}
            ;;
        --help|-h)
            usage
            exit 0
            ;;
        *)
            fail "unknown option: $1"
            ;;
    esac
    shift
done

[[ -n "$fixtures_dir" ]] || fail "--fixtures-dir is required"
[[ -d "$fixtures_dir" ]] || fail "fixtures directory does not exist: $fixtures_dir"
[[ -n "$output_dir" ]] || fail "--output-dir is required"
[[ "$runs" =~ ^[0-9]+$ && "$runs" -gt 0 ]] || fail "--runs must be a positive integer"
[[ -f "$measure_script" ]] || fail "measure_run.sh not found: $measure_script"

python_bin=$(resolve_python) || fail "python3 or python is required"
mnemosyne_bin=""
mat_bin=""
hprof_slurp_bin=""

split_csv "$tools_csv" selected_tools
split_csv "$fixtures_csv" selected_fixtures

[[ ${#selected_tools[@]} -gt 0 ]] || fail "no tools selected"
[[ ${#selected_fixtures[@]} -gt 0 ]] || fail "no fixtures selected"

available_tools=()
need_mnemo=0

for tool_name in "${selected_tools[@]}"; do
    case "$tool_name" in
        mnemo-deep|mnemo-overview)
            need_mnemo=1
            available_tools+=("$tool_name")
            ;;
        mat)
            if mat_bin=$(resolve_mat_bin); then
                export MAT_VMARGS=${MAT_VMARGS:--Xmx16g}
                available_tools+=("$tool_name")
            else
                warn "skipping mat because MAT_HOME/ParseHeapDump.sh is unavailable"
            fi
            ;;
        hprof-slurp)
            if hprof_slurp_bin=$(resolve_hprof_slurp_bin); then
                available_tools+=("$tool_name")
            else
                warn "skipping hprof-slurp because it is not on PATH"
            fi
            ;;
        *)
            fail "unsupported tool in --tools: $tool_name"
            ;;
    esac
done

if (( need_mnemo == 1 )); then
    mnemosyne_bin=$(resolve_mnemosyne_bin) || fail "mnemosyne binary not found; set MNEMOSYNE_BIN or build target/release/mnemosyne-cli"
fi

[[ ${#available_tools[@]} -gt 0 ]] || fail "no runnable tools remain after prerequisite checks"

mkdir -p "$output_dir"
runs_dir="$output_dir/runs"
mkdir -p "$runs_dir"
results_csv="$output_dir/results.csv"

printf 'fixture,fixture_size_bytes,tool,run_index,wall_time_seconds,max_rss_kb,exit_code,status,timestamp_utc\n' >"$results_csv"

for fixture_label in "${selected_fixtures[@]}"; do
    fixture_path=$(fixture_path_for_label "$fixture_label")

    for tool_name in "${available_tools[@]}"; do
        for (( run_index = 1; run_index <= runs; run_index++ )); do
            prepare_tool_command "$tool_name" "$fixture_path"
            run_label="${fixture_label}-${tool_name}-run${run_index}"

            if [[ "$tool_name" == "mat" ]]; then
                cleanup_mat_artifacts "$fixture_path"
            fi

            json_line=$("$measure_script" \
                --tool "$tool_name" \
                --fixture "$fixture_path" \
                --output-dir "$runs_dir" \
                --label "$run_label" \
                --timeout-seconds "$TOOL_TIMEOUT_SECONDS" \
                -- "${TOOL_COMMAND[@]}")

            append_csv_row "$fixture_label" "$run_index" "$json_line"
        done
    done
done

print_summary_table