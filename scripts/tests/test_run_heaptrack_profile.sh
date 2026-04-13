#!/usr/bin/env bash

set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
shell_bin=$(command -v bash)
tmpdir=$(mktemp -d)
stub_dir="$tmpdir/stubs"

cleanup() {
    rm -rf "$tmpdir"
}

trap cleanup EXIT

heap_path="$tmpdir/test.hprof"
: > "$heap_path"

output=$(PATH="$tmpdir" "$shell_bin" "$repo_root/scripts/run_heaptrack_profile.sh" "$heap_path" 2>&1)
printf '%s\n' "$output"

if ! grep -q 'heaptrack not installed' <<<"$output"; then
    echo "expected missing-heaptrack skip message" >&2
    exit 1
fi

mkdir -p "$stub_dir"

cat >"$stub_dir/heaptrack" <<EOF
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "\$*" > "$tmpdir/heaptrack.args"
EOF

cat >"$stub_dir/mnemosyne-cli" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF

chmod +x "$stub_dir/heaptrack" "$stub_dir/mnemosyne-cli"

profile_output="$tmpdir/custom.heaptrack.gz"
invocation_output=$(PATH="$stub_dir:$PATH" "$shell_bin" "$repo_root/scripts/run_heaptrack_profile.sh" --output "$profile_output" --command "analyze --threads --strings --collections" "$heap_path" 2>&1)
printf '%s\n' "$invocation_output"

if [[ ! -f "$tmpdir/heaptrack.args" ]]; then
    echo "expected heaptrack stub to capture argv" >&2
    exit 1
fi

captured_args=$(<"$tmpdir/heaptrack.args")

if [[ "$captured_args" != *"-o $profile_output "* ]]; then
    echo "expected heaptrack argv to include explicit output path" >&2
    exit 1
fi

if [[ "$captured_args" != *" analyze --threads --strings --collections $heap_path"* ]]; then
    echo "expected heaptrack argv to include output path and analyze command with heap" >&2
    exit 1
fi

if [[ "$invocation_output" != *"CLI command: "*" analyze --threads --strings --collections $heap_path"* ]]; then
    echo "expected wrapper to print the fully expanded CLI command" >&2
    exit 1
fi

if [[ "$invocation_output" != *"Heaptrack output: $profile_output"* ]]; then
    echo "expected wrapper to print chosen heaptrack output path" >&2
    exit 1
fi

echo "run_heaptrack_profile smoke test passed"
