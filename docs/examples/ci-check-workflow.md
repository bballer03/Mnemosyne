# CI Check Workflow

Scenario: your CI job already captures one or more `.hprof` files and you want Mnemosyne itself to decide whether the build should pass. This flow keeps the policy file small, produces machine-readable artifacts, and still leaves room for a richer `analyze` artifact when you need one.

## 1. Write A Policy File

```toml
[meta]
name = "checkout-service"

[defaults]
severity = "error"

[[rule]]
id = "heap-budget"
predicate = "total_bytes"
op = "<="
value = 2147483648

[[rule]]
id = "no-critical-leaks"
predicate = "leak_count"
op = "=="
value = 0
severity = "critical"
severity_filter = "critical"
```

The current policy surface supports 10 predicates: seven overview-compatible predicates plus deep-only `leak_count`, `retained_size`, and `dominator_root_count`. For the full schema and predicate catalog, see [../design/milestone-7-2-ci-regression-policies.md](../design/milestone-7-2-ci-regression-policies.md).

## 2. Run The Gate Locally

```bash
mnemosyne-cli ci-check heap.hprof --policy policy.toml --fail-on error
```

Use this when you want the same pass/fail behavior locally that CI will enforce later.

## 3. Persist Machine-Readable Artifacts

```bash
mnemosyne-cli ci-check heap.hprof --policy policy.toml --format json --output heap-policy.json
mnemosyne-cli ci-check heap.hprof --policy policy.toml --format junit --output heap-policy.xml
```

`json` is useful for dashboards or custom follow-on tooling. `junit` is useful when your CI already knows how to display test reports.

## 4. Emit GitHub Actions Annotations

```bash
mnemosyne-cli ci-check heap.hprof --policy policy.toml --format github-actions --fail-on warning
```

This emits workflow commands with `error`, `warning`, or `notice` severity plus a plain summary line. The current runtime includes `file=` but not `line=` because policy source spans are not tracked yet.

## 5. Pair It With `analyze` When You Need More Context

```bash
mnemosyne-cli analyze heap.hprof --profile ci-regression --format json --output-file analysis.json
mnemosyne-cli ci-check heap.hprof --policy policy.toml --format junit --output heap-policy.xml
```

Use `analyze --profile ci-regression` when you want a richer archived artifact. Use `ci-check` when you want the deterministic pass/fail contract.

## Notes

- The severity ladder is `info < warning < error < critical`.
- `--fail-on` defaults to `error` and changes the process exit code only; the rendered reports still show every violation and skipped rule.
- Exit codes are `0` clean or below threshold, `1` violation at or above `--fail-on`, `2` invalid policy, `3` unreadable heap or analysis failure, and `4` explicit `--mode overview` with a deep-only rule.
- `--mode auto` may resolve to overview and skip deep-only rules; explicit `--mode overview` with a deep-only policy returns exit `4`.