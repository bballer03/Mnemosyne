# Mnemosyne in Jenkins

This guide shows practical Jenkins pipeline patterns for running Mnemosyne against committed or generated `.hprof` files.

For the full CLI surface, see [the user guide](../user-guide.md). For install channels and release artifacts, see [the repository README](../../README.md).

## Before you start

- The current checked-in CLI exposes structured JSON on `mnemosyne-cli analyze --format json`.
- The current checked-in CLI does not expose `--format` on `leaks` or `diff`, so Jenkins gates should parse `analyze` JSON and treat `leaks` or `diff` output as human-readable artifacts.
- Mnemosyne returns `0` when analysis completes successfully. If you want Jenkins to fail on findings, add explicit Groovy or shell checks that call `error(...)` or return a non-zero exit code.
- These examples assume Linux agents.

## 1. Pipeline Setup

This declarative pipeline installs a published Mnemosyne release binary into the workspace and writes a JSON report.

```groovy
pipeline {
  agent any

  environment {
    // Replace this with the committed or generated heap dump path.
    HEAP_FILE = 'fixtures/app.hprof'
    REPORT_DIR = 'reports'
    MNEMOSYNE_VERSION = '0.2.0'
  }

  stages {
    stage('Checkout') {
      steps {
        // Pull the repository so Jenkins can read the heap dump and write reports.
        checkout scm
      }
    }

    stage('Install Mnemosyne') {
      steps {
        sh '''
          set -euo pipefail
          mkdir -p "$WORKSPACE/bin" "$REPORT_DIR"
          curl -L "https://github.com/bballer03/mnemosyne/releases/download/v${MNEMOSYNE_VERSION}/mnemosyne-cli-x86_64-unknown-linux-gnu.tar.gz" -o mnemosyne-cli.tar.gz
          tar -xzf mnemosyne-cli.tar.gz
          install -m 0755 mnemosyne-cli "$WORKSPACE/bin/mnemosyne-cli"
        '''
      }
    }

    stage('Analyze heap') {
      steps {
        sh '''
          set -euo pipefail
          "$WORKSPACE/bin/mnemosyne-cli" analyze "$HEAP_FILE" \
            --profile ci-regression \
            --format json \
            --output-file "$REPORT_DIR/analysis.json"
        '''
      }
    }
  }
}
```

If your agents already have Rust installed and you prefer `cargo install`, replace the install stage with `cargo install mnemosyne-cli --locked --version 0.2.0`.

## 2. Docker Agent

This example uses a Docker-capable Jenkins agent and runs the published Mnemosyne image directly.

```groovy
pipeline {
  // This Jenkins node needs Docker CLI access.
  agent { label 'docker' }

  environment {
    // Replace this with the committed or generated heap dump path.
    HEAP_FILE = 'fixtures/app.hprof'
    REPORT_DIR = 'reports'
  }

  stages {
    stage('Checkout') {
      steps {
        checkout scm
      }
    }

    stage('Analyze with published container') {
      steps {
        sh '''
          set -euo pipefail
          mkdir -p "$REPORT_DIR"
          docker pull ghcr.io/bballer03/mnemosyne:0.2.0
          docker run --rm \
            -v "$WORKSPACE:/workspace" \
            -w /workspace \
            ghcr.io/bballer03/mnemosyne:0.2.0 \
            analyze "$HEAP_FILE" \
            --profile ci-regression \
            --format json \
            --output-file "$REPORT_DIR/analysis.json"
        '''
      }
    }
  }
}
```

Note: the published Docker image sets `ENTRYPOINT ["mnemosyne-cli"]`. Running it explicitly with `docker run ... analyze ...` avoids Jenkins plugin edge cases around entrypoints.

## 3. Leak Detection Stage

Use a Groovy stage like this after `reports/analysis.json` exists.

```groovy
stage('Leak gate') {
  environment {
    // Allow zero high-severity graph-backed findings by default.
    MAX_HIGH_LEAKS = '0'
  }

  steps {
    script {
      // Parse the AnalyzeResponse JSON with standard Groovy.
      def analysis = new groovy.json.JsonSlurperClassic().parseText(readFile('reports/analysis.json'))

      // Keep only HIGH or CRITICAL findings with no provenance markers,
      // which usually means the result came from the preferred graph-backed path.
      def highLeaks = analysis.leaks.findAll { leak ->
        ['HIGH', 'CRITICAL'].contains(leak.severity) && (!leak.provenance || leak.provenance.isEmpty())
      }

      echo "High/Critical graph-backed leak count: ${highLeaks.size()}"

      if (highLeaks.size() > env.MAX_HIGH_LEAKS.toInteger()) {
        highLeaks.each { leak ->
          echo "${leak.severity} ${leak.id} ${leak.class_name} retained=${leak.retained_size_bytes} score=${leak.suspect_score ?: 'n/a'}"
        }

        // Fail the stage explicitly because Mnemosyne itself exits 0 on successful analysis.
        error('Mnemosyne leak gate failed')
      }
    }
  }
}
```

If you want to include fallback or synthetic findings in the gate, remove the provenance check from `highLeaks`.

## 4. Report Archiving

This pattern renders an HTML report, keeps the JSON report, and archives both into Jenkins build artifacts.

```groovy
stage('Render reports') {
  steps {
    sh '''
      set -euo pipefail
      "$WORKSPACE/bin/mnemosyne-cli" analyze "$HEAP_FILE" \
        --profile ci-regression \
        --format json \
        --output-file "$REPORT_DIR/analysis.json"
      "$WORKSPACE/bin/mnemosyne-cli" analyze "$HEAP_FILE" \
        --format html \
        --output-file "$REPORT_DIR/analysis.html"
    '''
  }
}

stage('Archive Mnemosyne reports') {
  steps {
    // Archive the reports even if a later stage fails.
    archiveArtifacts artifacts: 'reports/*.json, reports/*.html, reports/*.txt', fingerprint: true, onlyIfSuccessful: false
  }
}
```

If you use the Jenkins HTML Publisher plugin, you can publish `reports/analysis.html` in addition to archiving it.

## 5. Heap regression checks with `mnemosyne ci-check`

Use `mnemosyne-cli ci-check` when the Jenkins pipeline should fail based on a checked-in policy file instead of ad hoc Groovy thresholds. `analyze --profile ci-regression` still produces a richer artifact, but `ci-check` is the deterministic gate and JUnit producer.

```groovy
stage('Heap regression checks') {
  steps {
    script {
      int ciStatus = sh(
        returnStatus: true,
        script: '''
          set +e
          "$WORKSPACE/bin/mnemosyne-cli" ci-check "$HEAP_FILE" \
            --policy .mnemosyne/policy.toml \
            --format json \
            --output mnemosyne-policy.json
          status=$?

          "$WORKSPACE/bin/mnemosyne-cli" ci-check "$HEAP_FILE" \
            --policy .mnemosyne/policy.toml \
            --format junit \
            --output mnemosyne-policy.xml
          junit_status=$?

          if [ "$status" -ne 0 ]; then
            exit "$status"
          fi

          exit "$junit_status"
        '''
      )

      junit testResults: 'mnemosyne-policy.xml', allowEmptyResults: false
      archiveArtifacts artifacts: 'mnemosyne-policy.json, mnemosyne-policy.xml', fingerprint: true, onlyIfSuccessful: false

      if (ciStatus != 0) {
        error("Mnemosyne ci-check failed with exit code ${ciStatus}")
      }
    }
  }
}
```

Exit codes from `mnemosyne-cli ci-check`:

- `0` - policy clean, or only violations below `--fail-on`
- `1` - at least one violation met or exceeded `--fail-on`
- `2` - invalid policy file or schema error
- `3` - unreadable heap dump or analysis failure
- `4` - explicit `--mode overview` with a deep-only policy rule

Notes:

- `junit 'mnemosyne-policy.xml'` makes each policy rule appear as one test case in Jenkins test reporting.
- Keep archiving the JSON output even when Jenkins also ingests JUnit. The JSON file is the easier artifact for later diffing or dashboard ingestion.
- If you already run `mnemosyne-cli analyze --profile ci-regression`, keep that stage for richer artifacts. `ci-check` is the policy gate, not a replacement for the broader analysis report.