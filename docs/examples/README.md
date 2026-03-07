# Examples

This directory contains practical examples of using Mnemosyne.

## Example Files

- [basic-analysis.md](basic-analysis.md) - Simple heap dump analysis
- [ci-integration.md](ci-integration.md) - CI/CD pipeline integration
- [leak-investigation.md](leak-investigation.md) - Step-by-step leak debugging
- [mcp-usage.md](mcp-usage.md) - Using Mnemosyne via MCP in your IDE

## Sample Heap Dumps

Small example heap dumps for testing:

- `small-leak.hprof` - Small heap with obvious leak (5 MB)
- `thread-leak.hprof` - Thread leak example (12 MB)
- `cache-growth.hprof` - Unbounded cache example (8 MB)

Run `mnemosyne parse <dump>` against any of them to see the real class histogram and record-tag percentages that now power leak heuristics, diff output, and dominator summaries.

## Configuration Examples

Sample configuration files for different scenarios:

### Production Monitoring

```toml
# configs/production.toml
[general]
output_format = "json"

[analysis]
min_severity = "HIGH"
enable_ai = true

[llm]
provider = "openai"
model = "gpt-4"
```

### CI/CD Pipeline

```toml
# configs/ci.toml
[general]
output_format = "json"
verbose = false

[analysis]
enable_ai = false
min_severity = "CRITICAL"

[parser]
threads = 8
```

### Local Development

```toml
# configs/dev.toml
[general]
output_format = "text"
verbose = true

[analysis]
enable_ai = true
packages = ["com.myapp"]

[llm]
provider = "local"
endpoint = "http://localhost:11434/v1"
model = "llama2"
```

## Scripts

Useful scripts for automation:

### Memory Regression Test

```bash
#!/bin/bash
# check-memory-regression.sh

BEFORE=$1
AFTER=$2
THRESHOLD=100  # MB

RAW_DELTA=$(mnemosyne diff "$BEFORE" "$AFTER" | awk '/Delta size/ {print $4}')
DELTA_MB=${RAW_DELTA:+${RAW_DELTA#+}}
DELTA_MB=${DELTA_MB:-0}

if (( $(echo "$DELTA_MB > $THRESHOLD" | bc -l) )); then
  echo "❌ Memory regression detected: +${DELTA_MB}MB"
  exit 1
else
  echo "✓ Memory usage within acceptable range: +${DELTA_MB}MB"
  exit 0
fi
```

### Automated Leak Detection

```bash
#!/bin/bash
# nightly-leak-check.sh

PID=$(pgrep -f "MyApplication")
TIMESTAMP=$(date +%Y%m%d-%H%M%S)
HEAP="heap-${TIMESTAMP}.hprof"

# Take heap dump
jmap -dump:format=b,file=$HEAP $PID

# Analyze
mnemosyne analyze $HEAP --format toon --min-severity HIGH > report-${TIMESTAMP}.toon

# Check for critical leaks
if grep -q "severity=Critical" report-${TIMESTAMP}.toon; then
  # Send alert
  curl -X POST https://api.pagerduty.com/incidents \
    -H "Authorization: Token ${PAGERDUTY_TOKEN}" \
    -d "@alert-payload.json"
fi

# Cleanup old dumps (keep last 7 days)
find . -name "heap-*.hprof" -mtime +7 -delete
```

## Integration Examples

### GitHub Actions

```yaml
# .github/workflows/memory-check.yml
name: Memory Leak Detection

on:
  pull_request:
    branches: [main]

jobs:
  memory-analysis:
    runs-on: ubuntu-latest
    
    steps:
      - uses: actions/checkout@v3
      
      - name: Install Mnemosyne
        run: |
          cargo install mnemosyne
      
      - name: Run Tests and Capture Heap Dump
        run: |
          ./gradlew test -Dheapdump.on.exit=true
      
      - name: Analyze Heap Dump
        run: |
          mnemosyne analyze build/heap-dump.hprof \
            --format toon \
            --min-severity HIGH \
            > memory-report.toon
      
      - name: Check for Critical Leaks
        run: |
          if grep -q "severity=Critical" memory-report.toon; then
            echo "::error::Critical memory leak detected!"
            exit 1
          fi
      
      - name: Upload Report
        uses: actions/upload-artifact@v3
        with:
          name: memory-report
          path: memory-report.toon
```

### Kubernetes CronJob

```yaml
# k8s-memory-monitor.yaml
apiVersion: batch/v1
kind: CronJob
metadata:
  name: mnemosyne-memory-monitor
spec:
  schedule: "0 */6 * * *"  # Every 6 hours
  jobTemplate:
    spec:
      template:
        spec:
          containers:
          - name: mnemosyne
            image: mnemosyne:latest
            env:
            - name: OPENAI_API_KEY
              valueFrom:
                secretKeyRef:
                  name: mnemosyne-secrets
                  key: openai-api-key
            command:
            - /bin/bash
            - -c
            - |
              # Get pod name of target application
              POD=$(kubectl get pod -l app=myapp -o jsonpath='{.items[0].metadata.name}')
              
              # Exec into pod and take heap dump
              kubectl exec $POD -- jmap -dump:format=b,file=/tmp/heap.hprof 1
              
              # Copy heap dump
              kubectl cp $POD:/tmp/heap.hprof ./heap.hprof
              
              # Analyze
              mnemosyne analyze heap.hprof --ai --format toon > report.toon
              
              # Send to monitoring system
              curl -X POST https://monitoring.example.com/api/memory-reports \
                -H "Content-Type: text/plain" \
                -d @report.toon
```

## See Also

- [Quick Start Guide](../QUICKSTART.md)
- [API Reference](../api.md)
- [Configuration Guide](../configuration.md)
