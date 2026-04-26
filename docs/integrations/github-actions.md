# Mnemosyne in GitHub Actions

This guide shows copy-paste-ready GitHub Actions workflows for running Mnemosyne against committed or generated `.hprof` files.

For the full CLI surface, see [the user guide](../user-guide.md). For install channels and published artifacts, see [the repository README](../../README.md).

## Before you start

- These examples assume a Linux runner and a sanitized heap dump that is either committed to the repo or generated earlier in the workflow.
- Do not commit production heap dumps or dumps with customer data, secrets, or other sensitive content.
- The current checked-in CLI exposes structured JSON on `mnemosyne-cli analyze --format json`.
- The current checked-in CLI does not expose `--format` on `leaks` or `diff`, so automated CI gates should parse `analyze` JSON and treat `leaks` or `diff` output as human-readable artifacts.
- Mnemosyne exits with `0` when analysis completes successfully. To fail a build on leak counts or growth thresholds, add your own `jq` checks and `exit 1` logic.

## 1. Quick Start

Use this minimal workflow when you already have a committed heap dump in the repository and you want one machine-readable report per run.

```yaml
name: Mnemosyne Heap Analysis

on:
  pull_request:
  push:
    branches: [main]

env:
  # Replace this with the committed heap dump path in your repository.
  HEAP_FILE: fixtures/app.hprof

jobs:
  analyze-heap:
    runs-on: ubuntu-latest
    steps:
      - name: Checkout code
        uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1

      - name: Install Rust toolchain for cargo install
        uses: dtolnay/rust-toolchain@631a55b12751854ce901bb631d5902ceb48146f7 # stable

      - name: Cache cargo registry and build artifacts
        uses: Swatinem/rust-cache@779680da715d629ac1d338a641029a2f4372abb5 # v2.8.2

      - name: Install Mnemosyne CLI from crates.io
        run: cargo install mnemosyne-cli --locked --version 0.2.0

      - name: Run CI regression profile and save JSON
        run: |
          set -euo pipefail
          mkdir -p reports
          mnemosyne-cli analyze "$HEAP_FILE" \
            --profile ci-regression \
            --format json \
            --output-file reports/analysis.json
```

If you prefer a prebuilt release binary instead of `cargo install`, replace the install step with this one:

```yaml
      - name: Install Mnemosyne release binary
        env:
          VERSION: 0.2.0
        run: |
          set -euo pipefail
          mkdir -p "$HOME/.local/bin"
          curl -L "https://github.com/bballer03/mnemosyne/releases/download/v${VERSION}/mnemosyne-cli-x86_64-unknown-linux-gnu.tar.gz" -o mnemosyne-cli.tar.gz
          tar -xzf mnemosyne-cli.tar.gz
          install -m 0755 mnemosyne-cli "$HOME/.local/bin/mnemosyne-cli"
          echo "$HOME/.local/bin" >> "$GITHUB_PATH"
```

## 2. Regression Detection

Use `diff` when you want a reviewer-facing before and after comparison for the same heap path across commits.

Because `diff` is text-only in the current CLI, this pattern produces a readable artifact rather than a structured gate.

```yaml
name: Mnemosyne Heap Diff

on:
  pull_request:

env:
  # Replace this with the committed heap dump path in your repository.
  HEAP_FILE: fixtures/app.hprof

jobs:
  diff-heap:
    runs-on: ubuntu-latest
    steps:
      - name: Checkout code and parent commit
        uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1
        with:
          # fetch-depth 2 is enough when you want the immediate parent commit.
          fetch-depth: 2

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@631a55b12751854ce901bb631d5902ceb48146f7 # stable

      - name: Cache cargo registry and build artifacts
        uses: Swatinem/rust-cache@779680da715d629ac1d338a641029a2f4372abb5 # v2.8.2

      - name: Install Mnemosyne CLI
        run: cargo install mnemosyne-cli --locked --version 0.2.0

      - name: Extract the previous and current heap dumps
        run: |
          set -euo pipefail
          mkdir -p reports tmp

          # Skip cleanly when the parent commit does not contain the same heap file.
          if ! git cat-file -e "HEAD^:$HEAP_FILE" 2>/dev/null; then
            echo "No previous heap dump found at $HEAP_FILE in the parent commit; skipping diff."
            exit 0
          fi

          git show "HEAD^:$HEAP_FILE" > tmp/before.hprof
          cp "$HEAP_FILE" tmp/after.hprof

      - name: Generate a human-readable heap diff report
        run: |
          set -euo pipefail
          if [ ! -f tmp/before.hprof ]; then
            exit 0
          fi
          mnemosyne-cli diff tmp/before.hprof tmp/after.hprof | tee reports/diff.txt
```

Notes:

- For pull requests, replace `HEAD^` with `${{ github.event.pull_request.base.sha }}` if you want to diff against the PR base instead of the previous commit.
- For machine gating, pair this with two `analyze --format json` runs and compare the fields you care about with `jq` or a small script.

## 3. Leak Gate

Use this pattern when the build should fail if high-severity leak suspects exceed a threshold.

The current CLI does not expose `--format json` on `leaks`, so the gate below parses the `.leaks[]` array from `AnalyzeResponse` instead.

```yaml
name: Mnemosyne Leak Gate

on:
  pull_request:
  push:
    branches: [main]

env:
  # Replace this with the committed or generated heap dump path.
  HEAP_FILE: fixtures/app.hprof
  # Set the maximum number of HIGH or CRITICAL graph-backed findings you allow.
  MAX_HIGH_LEAKS: "0"

jobs:
  leak-gate:
    runs-on: ubuntu-latest
    steps:
      - name: Checkout code
        uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@631a55b12751854ce901bb631d5902ceb48146f7 # stable

      - name: Cache cargo registry and build artifacts
        uses: Swatinem/rust-cache@779680da715d629ac1d338a641029a2f4372abb5 # v2.8.2

      - name: Install Mnemosyne CLI
        run: cargo install mnemosyne-cli --locked --version 0.2.0

      - name: Produce structured analysis JSON
        run: |
          set -euo pipefail
          mkdir -p reports
          mnemosyne-cli analyze "$HEAP_FILE" \
            --profile ci-regression \
            --format json \
            --output-file reports/analysis.json

      - name: Fail the build when leak suspects exceed the threshold
        run: |
          set -euo pipefail

          # This filter keeps only HIGH or CRITICAL findings with no provenance markers,
          # which usually means the result came from the preferred graph-backed path.
          high_leaks="$(jq '[.leaks[] | select((.severity == "HIGH" or .severity == "CRITICAL") and ((.provenance | length) == 0))] | length' reports/analysis.json)"

          echo "High/Critical graph-backed leak count: $high_leaks"

          if [ "$high_leaks" -gt "$MAX_HIGH_LEAKS" ]; then
            echo "Mnemosyne leak gate failed."
            jq -r '.leaks[]
              | select((.severity == "HIGH" or .severity == "CRITICAL") and ((.provenance | length) == 0))
              | "\(.severity)\t\(.id)\t\(.class_name)\tretained=\(.retained_size_bytes)\tscore=\(.suspect_score // \"n/a\")"' reports/analysis.json
            exit 1
          fi
```

If you want to include fallback or synthetic findings in the gate, remove the `((.provenance | length) == 0)` clause from the `jq` filter.

## 4. Docker-based

Use the published container image when you want a reproducible runtime and do not want to compile Rust code on every runner.

```yaml
name: Mnemosyne Heap Analysis via Docker

on:
  workflow_dispatch:
  pull_request:

env:
  # Replace this with the committed or generated heap dump path.
  HEAP_FILE: fixtures/app.hprof

jobs:
  analyze-in-container:
    runs-on: ubuntu-latest
    steps:
      - name: Checkout code
        uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1

      - name: Pull the published Mnemosyne image
        run: docker pull ghcr.io/bballer03/mnemosyne:0.2.0

      - name: Run analysis inside the container
        run: |
          set -euo pipefail
          mkdir -p reports
          docker run --rm \
            -v "$GITHUB_WORKSPACE:/workspace" \
            -w /workspace \
            ghcr.io/bballer03/mnemosyne:0.2.0 \
            analyze "$HEAP_FILE" \
            --profile ci-regression \
            --format json \
            --output-file reports/analysis.json
```

This works well when the heap dump already lives in the workspace or was downloaded into the runner earlier in the job.

## 5. Matrix builds

Use a matrix when you want one job definition to analyze multiple heap dumps.

```yaml
name: Mnemosyne Matrix Analysis

on:
  pull_request:

jobs:
  analyze-many-heaps:
    runs-on: ubuntu-latest
    strategy:
      fail-fast: false
      matrix:
        include:
          - heap_file: fixtures/service-a.hprof
            report_name: service-a
          - heap_file: fixtures/service-b.hprof
            report_name: service-b

    steps:
      - name: Checkout code
        uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@631a55b12751854ce901bb631d5902ceb48146f7 # stable

      - name: Cache cargo registry and build artifacts
        uses: Swatinem/rust-cache@779680da715d629ac1d338a641029a2f4372abb5 # v2.8.2

      - name: Install Mnemosyne CLI
        run: cargo install mnemosyne-cli --locked --version 0.2.0

      - name: Analyze the current heap dump from the matrix
        run: |
          set -euo pipefail
          mkdir -p reports
          mnemosyne-cli analyze "${{ matrix.heap_file }}" \
            --profile ci-regression \
            --format json \
            --output-file "reports/${{ matrix.report_name }}.json"

      - name: Upload the per-heap report artifact
        uses: actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02 # v4.6.2
        with:
          name: mnemosyne-${{ matrix.report_name }}
          path: reports/${{ matrix.report_name }}.json
          if-no-files-found: error
```

## 6. Artifact upload

Use artifact uploads when you want JSON or HTML reports to survive the job and be downloadable from the Actions UI.

```yaml
      - name: Generate JSON and HTML reports
        run: |
          set -euo pipefail
          mkdir -p reports
          mnemosyne-cli analyze "$HEAP_FILE" \
            --profile ci-regression \
            --format json \
            --output-file reports/analysis.json
          mnemosyne-cli analyze "$HEAP_FILE" \
            --format html \
            --output-file reports/analysis.html

      - name: Upload Mnemosyne report artifacts
        if: always()
        uses: actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02 # v4.6.2
        with:
          name: mnemosyne-reports
          path: reports/
          if-no-files-found: error
```

`if: always()` is useful when you still want the report artifacts even after a leak gate fails.

## 7. Complete example

This workflow combines JSON analysis, an optional human-readable diff report, a `jq` leak gate, and artifact uploads.

```yaml
name: Mnemosyne Heap CI

on:
  pull_request:
  push:
    branches: [main]

permissions:
  contents: read

env:
  # Replace this with the committed or generated heap dump path.
  HEAP_FILE: fixtures/app.hprof
  MAX_HIGH_LEAKS: "0"

jobs:
  heap-analysis:
    runs-on: ubuntu-latest
    steps:
      - name: Checkout code and parent commit
        uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1
        with:
          # Needed for the optional diff step.
          fetch-depth: 2

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@631a55b12751854ce901bb631d5902ceb48146f7 # stable

      - name: Cache cargo registry and build artifacts
        uses: Swatinem/rust-cache@779680da715d629ac1d338a641029a2f4372abb5 # v2.8.2

      - name: Install Mnemosyne CLI
        run: cargo install mnemosyne-cli --locked --version 0.2.0

      - name: Create report directories
        run: mkdir -p reports tmp

      - name: Generate JSON and HTML reports
        run: |
          set -euo pipefail
          mnemosyne-cli analyze "$HEAP_FILE" \
            --profile ci-regression \
            --format json \
            --output-file reports/analysis.json
          mnemosyne-cli analyze "$HEAP_FILE" \
            --format html \
            --output-file reports/analysis.html

      - name: Generate a text diff report when the parent commit has the same heap path
        run: |
          set -euo pipefail
          if git cat-file -e "HEAD^:$HEAP_FILE" 2>/dev/null; then
            git show "HEAD^:$HEAP_FILE" > tmp/before.hprof
            cp "$HEAP_FILE" tmp/after.hprof
            mnemosyne-cli diff tmp/before.hprof tmp/after.hprof | tee reports/diff.txt
          else
            echo "No previous committed heap dump found at $HEAP_FILE; skipping diff."
          fi

      - name: Fail the job when the high-severity leak threshold is exceeded
        run: |
          set -euo pipefail
          high_leaks="$(jq '[.leaks[] | select((.severity == "HIGH" or .severity == "CRITICAL") and ((.provenance | length) == 0))] | length' reports/analysis.json)"
          echo "High/Critical graph-backed leak count: $high_leaks"
          if [ "$high_leaks" -gt "$MAX_HIGH_LEAKS" ]; then
            echo "Mnemosyne leak gate failed."
            jq -r '.leaks[]
              | select((.severity == "HIGH" or .severity == "CRITICAL") and ((.provenance | length) == 0))
              | "\(.severity)\t\(.id)\t\(.class_name)\tretained=\(.retained_size_bytes)\tscore=\(.suspect_score // \"n/a\")"' reports/analysis.json
            exit 1
          fi

      - name: Upload Mnemosyne artifacts
        if: always()
        uses: actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02 # v4.6.2
        with:
          name: mnemosyne-heap-analysis
          path: reports/
          if-no-files-found: error
```

This example is a good starting point when you want one workflow to produce reviewer-facing output and still enforce an automated leak budget.

## 8. Heap regression checks with `mnemosyne ci-check`

Use `mnemosyne-cli ci-check` when you want Mnemosyne itself to own the pass/fail policy decision. `analyze --profile ci-regression` is still useful for producing a richer JSON artifact, but `ci-check` is the deterministic gate that turns policy violations into exit codes, JUnit XML, and GitHub Actions workflow commands.

The workflow below assumes an earlier job captures one or more `.hprof` files and uploads them as artifacts. The matrix fan-out then downloads those heaps, runs policy evaluation with GitHub Actions annotations, writes a JUnit XML report for each heap, uploads the reports, and finally fails each matrix item with the original `ci-check` exit code.

```yaml
name: Heap Regression Checks

on:
  pull_request:
  push:
    branches: [main]

jobs:
  capture-heaps:
    runs-on: ubuntu-latest
    outputs:
      heaps: ${{ steps.heap-list.outputs.heaps }}
    steps:
      - name: Checkout code
        uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1

      - name: Run the test job that writes heap dumps
        run: |
          set -euo pipefail
          mkdir -p heaps
          ./scripts/capture-test-heaps.sh heaps

      - id: heap-list
        shell: bash
        run: echo 'heaps=["checkout.hprof","search.hprof"]' >> "$GITHUB_OUTPUT"

      - name: Upload heap dump artifacts
        uses: actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02 # v4.6.2
        with:
          name: heap-dumps
          path: heaps/*.hprof

  heap-regression:
    needs: capture-heaps
    runs-on: ubuntu-latest
    strategy:
      fail-fast: false
      matrix:
        heap: ${{ fromJson(needs.capture-heaps.outputs.heaps) }}
    steps:
      - name: Checkout code
        uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@631a55b12751854ce901bb631d5902ceb48146f7 # stable

      - name: Cache cargo registry and build artifacts
        uses: Swatinem/rust-cache@779680da715d629ac1d338a641029a2f4372abb5 # v2.8.2

      - name: Install Mnemosyne CLI
        run: cargo install mnemosyne-cli --locked --version 0.2.0

      - name: Download heap dump artifacts
        uses: actions/download-artifact@d3f86a106a0bac45b974a628896c90dbdf5c8093 # v4.3.0
        with:
          name: heap-dumps
          path: heaps

      - name: Emit workflow annotations
        id: ci_check
        shell: bash
        run: |
          set +e
          mnemosyne-cli ci-check "heaps/${{ matrix.heap }}" \
            --policy .mnemosyne/policy.toml \
            --format github-actions
          status=$?
          echo "status=$status" >> "$GITHUB_OUTPUT"
          exit 0

      - name: Write JUnit XML report
        id: junit
        if: always()
        shell: bash
        run: |
          set +e
          mkdir -p reports
          mnemosyne-cli ci-check "heaps/${{ matrix.heap }}" \
            --policy .mnemosyne/policy.toml \
            --format junit \
            --output "reports/${{ matrix.heap }}.xml"
          echo "status=$?" >> "$GITHUB_OUTPUT"
          exit 0

      - name: Upload JUnit report artifact
        if: always()
        uses: actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02 # v4.6.2
        with:
          name: mnemosyne-junit-${{ matrix.heap }}
          path: reports/${{ matrix.heap }}.xml

      - name: Fail matrix item when the policy gate fails
        if: steps.ci_check.outputs.status != '0'
        shell: bash
        run: exit "${{ steps.ci_check.outputs.status }}"
```

Exit codes from `mnemosyne-cli ci-check`:

- `0` - policy clean, or only violations below `--fail-on`
- `1` - at least one violation met or exceeded `--fail-on`
- `2` - invalid policy file or schema error
- `3` - unreadable heap dump or analysis failure
- `4` - explicit `--mode overview` with a deep-only policy rule

Notes:

- The `github-actions` format intentionally emits `file=` but not `line=` today because policy source spans are not tracked yet.
- If you also want the richer analysis artifact, add a separate `mnemosyne-cli analyze --profile ci-regression --format json --output-file reports/analysis.json` step. That complements `ci-check`; it does not replace it.