#!/usr/bin/env bash

set -euo pipefail

DEFAULT_TIMEOUT_SECONDS=1800

usage() {
    cat >&2 <<'EOF'
usage: scripts/bench/measure_run.sh --tool <name> --fixture <path> --output-dir <dir> --label <run-label> [--timeout-seconds <seconds>] -- <command...>

Runs one tool invocation under GNU time, captures wall time + max RSS + exit code,
and writes:
  <output-dir>/<label>.json
  <output-dir>/<label>.stdout
  <output-dir>/<label>.stderr

The wrapper returns 0 even when the measured command exits non-zero or times out.
It returns non-zero only when wrapper setup or measurement fails.
EOF
}

fail() {
    printf 'error: %s\n' "$1" >&2
    exit 1
}

json_escape() {
    local value=$1
    value=${value//\\/\\\\}
    value=${value//"/\\"}
    value=${value//$'\n'/\\n}
    value=${value//$'\r'/\\r}
    value=${value//$'\t'/\\t}
    printf '%s' "$value"
}

quote_command() {
    local result=""
    local argument=""
    local quoted=""

    for argument in "$@"; do
        printf -v quoted '%q' "$argument"
        if [[ -n "$result" ]]; then
            result+=" "
        fi
        result+="$quoted"
    done

    printf '%s' "$result"
}

resolve_time_tool() {
    if [[ -x /usr/bin/time ]] && /usr/bin/time -v true >/dev/null 2>&1; then
        printf '%s\n' /usr/bin/time
        return 0
    fi

    if command -v gtime >/dev/null 2>&1 && gtime -v true >/dev/null 2>&1; then
        command -v gtime
        return 0
    fi

    return 1
}

resolve_timeout_tool() {
    if command -v timeout >/dev/null 2>&1; then
        command -v timeout
        return 0
    fi

    if command -v gtimeout >/dev/null 2>&1; then
        command -v gtimeout
        return 0
    fi

    return 1
}

require_executable() {
    local command_name=$1

    if [[ "$command_name" == */* ]]; then
        [[ -x "$command_name" ]] || fail "required executable not found: $command_name"
        return 0
    fi

    command -v "$command_name" >/dev/null 2>&1 || fail "required command not found on PATH: $command_name"
}

ensure_writable_dir() {
    local dir_path=$1
    local probe_file=""

    mkdir -p "$dir_path"
    probe_file="$dir_path/.measure-run-write-test.$$"
    : >"$probe_file" || fail "output directory is not writable: $dir_path"
    rm -f "$probe_file"
}

elapsed_to_seconds() {
    local value=$1

    awk -v value="$value" '
        BEGIN {
            count = split(value, parts, ":")
            if (count == 3) {
                printf "%.6f", (parts[1] * 3600.0) + (parts[2] * 60.0) + parts[3]
                exit 0
            }
            if (count == 2) {
                printf "%.6f", (parts[1] * 60.0) + parts[2]
                exit 0
            }
            if (count == 1) {
                printf "%.6f", parts[1]
                exit 0
            }
            exit 1
        }
    '
}

tool_name=""
fixture_path=""
output_dir=""
label=""
timeout_seconds=$DEFAULT_TIMEOUT_SECONDS
command_args=()

while [[ $# -gt 0 ]]; do
    case "$1" in
        --tool)
            shift
            tool_name=${1:-}
            ;;
        --fixture)
            shift
            fixture_path=${1:-}
            ;;
        --output-dir)
            shift
            output_dir=${1:-}
            ;;
        --label)
            shift
            label=${1:-}
            ;;
        --timeout-seconds)
            shift
            timeout_seconds=${1:-}
            ;;
        --help|-h)
            usage
            exit 0
            ;;
        --)
            shift
            command_args=("$@")
            break
            ;;
        *)
            fail "unknown option: $1"
            ;;
    esac
    shift
done

[[ -n "$tool_name" ]] || fail "--tool is required"
[[ -n "$fixture_path" ]] || fail "--fixture is required"
[[ -n "$output_dir" ]] || fail "--output-dir is required"
[[ -n "$label" ]] || fail "--label is required"
[[ ${#command_args[@]} -gt 0 ]] || fail "missing command after --"

[[ -f "$fixture_path" ]] || fail "fixture does not exist: $fixture_path"
[[ "$label" != *"/"* && "$label" != *"\\"* && "$label" != "." && "$label" != ".." ]] || fail "label must not contain path separators"
[[ "$timeout_seconds" =~ ^[0-9]+$ && "$timeout_seconds" -gt 0 ]] || fail "--timeout-seconds must be a positive integer"

ensure_writable_dir "$output_dir"
require_executable "${command_args[0]}"

time_tool=$(resolve_time_tool) || fail "GNU time with -v support is required; use /usr/bin/time on Linux or gtime on macOS"
timeout_tool=$(resolve_timeout_tool) || fail "timeout support is required; install GNU coreutils (timeout or gtimeout)"

stdout_file="$output_dir/$label.stdout"
stderr_file="$output_dir/$label.stderr"
json_file="$output_dir/$label.json"
time_file=$(mktemp)

cleanup() {
    rm -f "$time_file"
}

trap cleanup EXIT

fixture_size_bytes=$(wc -c <"$fixture_path")
timestamp_utc=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
command_string=$(quote_command "${command_args[@]}")

set +e
"$time_tool" -v -o "$time_file" "$timeout_tool" -k 10s "${timeout_seconds}s" -- "${command_args[@]}" >"$stdout_file" 2>"$stderr_file"
command_exit=$?
set -e

elapsed_raw=$(awk -F': ' '/Elapsed \(wall clock\) time/ {print $2; exit}' "$time_file")
max_rss_kb=$(awk -F': ' '/Maximum resident set size/ {gsub(/^[[:space:]]+/, "", $2); print $2; exit}' "$time_file")

[[ -n "$elapsed_raw" ]] || fail "failed to parse wall-clock time from GNU time output"
[[ -n "$max_rss_kb" ]] || fail "failed to parse maximum resident set size from GNU time output"

wall_time_seconds=$(elapsed_to_seconds "$elapsed_raw") || fail "failed to convert elapsed time '$elapsed_raw' to seconds"

status="ok"
if (( command_exit == 124 )); then
    status="timeout"
elif (( command_exit != 0 )); then
    status="error"
fi

json_line=$(printf '{"tool":"%s","fixture":"%s","fixture_size_bytes":%s,"label":"%s","status":"%s","wall_time_seconds":%s,"max_rss_kb":%s,"exit_code":%s,"timeout_seconds":%s,"timestamp_utc":"%s","stdout_path":"%s","stderr_path":"%s","json_path":"%s","command":"%s"}' \
    "$(json_escape "$tool_name")" \
    "$(json_escape "$fixture_path")" \
    "$fixture_size_bytes" \
    "$(json_escape "$label")" \
    "$(json_escape "$status")" \
    "$wall_time_seconds" \
    "$max_rss_kb" \
    "$command_exit" \
    "$timeout_seconds" \
    "$(json_escape "$timestamp_utc")" \
    "$(json_escape "$stdout_file")" \
    "$(json_escape "$stderr_file")" \
    "$(json_escape "$json_file")" \
    "$(json_escape "$command_string")")

printf '%s\n' "$json_line" | tee "$json_file"

exit 0