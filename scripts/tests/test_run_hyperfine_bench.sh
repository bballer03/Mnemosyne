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

output=$(PATH="$tmpdir" "$shell_bin" "$repo_root/scripts/run_hyperfine_bench.sh" "$heap_path" 2>&1)
printf '%s\n' "$output"

if ! grep -q 'hyperfine not installed' <<<"$output"; then
    echo "expected missing-hyperfine skip message" >&2
    exit 1
fi

mkdir -p "$stub_dir"

cat >"$stub_dir/hyperfine" <<EOF
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "\$*" > "$tmpdir/hyperfine.args"
EOF

cat >"$stub_dir/mnemosyne-cli" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF

chmod +x "$stub_dir/hyperfine" "$stub_dir/mnemosyne-cli"

invocation_output=$(PATH="$stub_dir:$PATH" "$shell_bin" "$repo_root/scripts/run_hyperfine_bench.sh" --commands "parse leaks" --runs 5 --warmup 2 "$heap_path" 2>&1)
printf '%s\n' "$invocation_output"

if [[ ! -f "$tmpdir/hyperfine.args" ]]; then
    echo "expected hyperfine stub to capture argv" >&2
    exit 1
fi

captured_args=$(<"$tmpdir/hyperfine.args")

if [[ "$captured_args" != *'--warmup 2 --runs 5'* ]]; then
    echo "expected hyperfine argv to include warmup and run counts" >&2
    exit 1
fi

if [[ "$captured_args" != *'--command-name parse'* ]]; then
    echo "expected hyperfine argv to include parse command label" >&2
    exit 1
fi

if [[ "$captured_args" != *'--command-name leaks'* ]]; then
    echo "expected hyperfine argv to include leaks command label" >&2
    exit 1
fi

if [[ "$captured_args" != *"mnemosyne-cli parse $heap_path"* ]]; then
    echo "expected hyperfine argv to include parse command string" >&2
    exit 1
fi

if [[ "$captured_args" != *"mnemosyne-cli leaks $heap_path"* ]]; then
    echo "expected hyperfine argv to include leaks command string" >&2
    exit 1
fi

echo "run_hyperfine_bench smoke test passed"
