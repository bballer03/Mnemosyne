# Mnemosyne Troubleshooting Guide

This guide covers the most common problems you are likely to hit with the current Mnemosyne CLI and MCP runtime, plus the practical workarounds that usually get you unblocked.

For the general product overview, see [../README.md](../README.md). For config details, see [configuration.md](configuration.md). For MCP request and response details, see [api.md](api.md). For contribution and bug-report expectations, see [../CONTRIBUTING.md](../CONTRIBUTING.md).

## 1. Common Errors

### "File not found"

Typical message:

```text
Error: File not found: heap
  hint: Did you mean 'heap.hprof'?
```

What it usually means:

- the path is wrong
- the filename is missing the `.hprof` extension
- you are running the command from a different working directory than you expected

What to do:

- rerun the command with an explicit absolute path to the dump
- check whether the file is really named `heap.hprof` instead of `heap`
- look for nearby `.hprof` files in the same directory; Mnemosyne already tries to suggest them when it can

Useful checks:

```bash
dir *.hprof
mnemosyne-cli parse C:\full\path\to\heap.hprof
```

### "Not a valid HPROF file"

Typical message:

```text
Error: Not a valid HPROF file: dump.txt
  hint: Expected an HPROF heap dump, but this file has a .txt extension.
```

What it usually means:

- you passed a log, CSV, text export, archive, or class file instead of an actual heap dump
- you pointed at a compressed artifact instead of the extracted `.hprof`

Common wrong file types the CLI already tries to recognize:

- `.jar`
- `.war`
- `.ear`
- `.zip`
- `.class`
- `.log`
- `.txt`
- `.csv`

What to do:

- verify that the input is the real dump file produced by `jmap` or your JVM's heap-dump mechanism
- if you downloaded a bundle, extract it first and point Mnemosyne at the `.hprof`
- if you only have an application artifact, recapture the heap dump from the running JVM

Recommended capture command:

```bash
jmap -dump:format=b,file=heap.hprof <PID>
```

### "HPROF header parse failure"

Typical message shape:

```text
Error: HPROF parse error (header): ...
```

What it usually means:

- the dump is corrupted or truncated
- the copy or download did not finish cleanly
- the file is not really an HPROF dump even though it has a `.hprof` extension

What to do:

- compare the file size with the source system's original dump
- recapture or recopy the dump if possible
- check whether the dump was partially written because the JVM or host ran out of space
- if you moved the file through another tool, verify that it did not compress, transform, or truncate it

Good first command:

```bash
mnemosyne-cli parse heap.hprof
```

If `parse` cannot get through the header, deeper commands will not help.

### "Invalid config file"

Typical message shape:

```text
Error: Configuration error: invalid TOML in '...'
  hint: Fix the TOML syntax in the config file and try again.
```

What it usually means:

- the config file has broken TOML syntax
- a value type is wrong for the current config schema
- the file path provided through `--config` or `MNEMOSYNE_CONFIG` does not exist

What to do:

- run `mnemosyne-cli config` without overrides to confirm that built-in defaults work
- then rerun with `--config /path/to/file.toml`
- check for trailing commas, mismatched quotes, or invalid enum values such as a misspelled provider name

Helpful pattern:

```bash
mnemosyne-cli config
mnemosyne-cli --config .mnemosyne.toml config
```

### "Unknown leak ID"

Typical message shape:

```text
Error: Invalid input: no leak found matching identifier 'missing::leak'
```

You can also see the same problem in chat mode as:

```text
Focus error: no leak found matching identifier 'missing::leak'
```

What it usually means:

- the leak ID came from an earlier run with different filters
- you copied a truncated display value instead of the full leak ID
- you are trying to explain or fix a leak that no longer exists under the current config

What to do:

- rerun `mnemosyne-cli leaks heap.hprof` with the same filters you plan to use for `explain` or `fix`
- if the table truncated the ID, copy it from the disclosure section that prints full values for truncated rows
- in chat mode, use `/list` before `/focus <leak-id>`

## 2. Performance Issues

### Large dumps take too long

What is happening:

- `parse` is the streaming, low-cost path
- `analyze`, `leaks`, `query`, and `gc-path` may need the graph-backed path
- investigation flags such as `--threads`, `--strings`, `--collections`, and `--top-instances` ask Mnemosyne to retain and inspect more data

What to do:

- start with `parse` first
- use plain `analyze` before enabling every investigation flag
- turn on deep modules only when you already have a concrete question
- use `--profile ci-regression` for CI instead of the full investigation set

Lean first-pass commands:

```bash
mnemosyne-cli parse heap.hprof
mnemosyne-cli analyze heap.hprof
mnemosyne-cli leaks heap.hprof
```

Investigation-heavy command:

```bash
mnemosyne-cli analyze heap.hprof --threads --strings --collections --top-instances
```

### High memory usage

What is happening:

- Mnemosyne's current analysis architecture is still in-memory
- the default graph-backed `analyze` and `leaks` path has been validated around roughly `2.87x-2.90x` RSS-to-dump ratio on dense synthetic large-dump tiers
- investigation-heavy runs have measured around `3.89x-3.92x` on the same tiers

What to do:

- treat `parse` as your first triage tool when memory headroom is tight
- avoid investigation flags in CI unless you need them for the specific regression signal
- close other large processes before running deep analysis on multi-GB dumps
- use a machine with comfortable free RAM for graph-backed investigation

Practical recommendation:

```text
Use `parse` first for quick triage. Move to `analyze` only after you know the dump is worth the deeper pass.
```

## 3. AI Provider Issues

### Missing API key

Typical message shapes:

```text
Error: Invalid input: missing AI provider API key in environment variable `OPENAI_API_KEY`
Error: Invalid input: Anthropic provider requires an API key
```

What it usually means:

- `mode = "provider"` is enabled, but the configured key environment variable is empty or unset
- you switched providers and forgot to update `api_key_env`

What to do:

- export the right environment variable before running the command
- or point `api_key_env` at the variable your environment actually uses

Examples:

```bash
export OPENAI_API_KEY="sk-..."
export ANTHROPIC_API_KEY="..."
```

### Provider timeout

Typical message shape:

```text
Error: AI provider request timed out: ...
```

What it usually means:

- the provider endpoint is slow or unreachable
- the timeout is too aggressive for the current model or network path
- a local provider process is running but overloaded

What to do:

- increase `[ai].timeout_secs`
- verify the `endpoint` URL
- confirm that the local gateway or remote provider is healthy
- switch temporarily to `mode = "rules"` to confirm the rest of the Mnemosyne workflow is fine

### Unsupported provider

Typical message shape:

```text
Configuration error: unsupported AI provider 'something-else'
```

Current supported values:

- `openai`
- `anthropic`
- `local`

What to do:

- change the provider name to one of the supported values
- if you are targeting a local OpenAI-compatible server, keep `provider = "local"` and set `endpoint`

### TOON parsing failures from provider responses

Typical message shapes:

```text
Error: Invalid input: provider TOON output missing response summary
Error: Invalid input: provider returned invalid confidence_pct
Error: Invalid input: provider TOON output missing section patch diff
```

What it usually means:

- the provider did not return the strict TOON structure Mnemosyne expects
- a gateway or local model wrapper rewrote the response into ordinary prose
- the provider returned an empty completion

What to do:

- test with `mode = "rules"` to confirm the rest of the command path works
- reduce custom prompt overrides while debugging
- if you are using a local or proxy endpoint, verify that it preserves the raw completion text instead of post-processing it
- for `fix`, remember that the provider response must include both a description and a patch diff section

## 4. MCP Issues

### Server not responding

What is usually wrong:

- the editor is not actually launching `mnemosyne-cli serve`
- the binary is not on the editor's PATH
- the client expects HTTP, but Mnemosyne serves over stdio

What to do:

- verify the exact command path the client is using
- run `mnemosyne-cli serve` manually in a terminal to confirm the binary works
- make sure the client is configured for stdio, not a socket or URL

Important runtime truth:

- `serve --host` and `--port` are accepted, but currently informational
- the transport contract is line-delimited JSON over stdin and stdout

### "Method not found"

Typical message shape:

```json
{
  "success": false,
  "error": "Invalid input: unsupported MCP method: nope",
  "error_details": {
    "code": "invalid_input"
  }
}
```

What it usually means:

- the client is calling the wrong method name
- the client assumes an older or broader contract than Mnemosyne currently exposes

What to do:

- call `list_tools` first and use the live catalog as the source of truth
- compare the client method name to [api.md](api.md)

Current method list includes:

- `list_tools`
- `parse_heap`
- `detect_leaks`
- `analyze_heap`
- `query_heap`
- `map_to_code`
- `find_gc_path`
- `create_ai_session`
- `resume_ai_session`
- `get_ai_session`
- `close_ai_session`
- `chat_session`
- `explain_leak`
- `propose_fix`

### Understanding `error_details`

Mnemosyne preserves a plain top-level `error` string for backward compatibility, but also returns a structured `error_details` object on failures.

Use it like this:

- `error_details.code`: stable machine-readable category such as `invalid_input` or `config_error`
- `error_details.message`: human-readable error string
- `error_details.details`: optional structured detail such as a bad method name, path, or suggestion

If you are writing an MCP client, use `error_details.code` for branching logic and `error` or `error_details.message` for operator-visible text.

## 5. Known Limitations

These are current runtime limitations, not documentation gaps.

### Object-level diff is not available yet

`diff` is useful today, but it is still aggregate in nature. You get record-level change and class-level retained deltas, not object-identity migration or reference-chain diffing.

Use it for:

- confirming whether a class grew or shrank
- comparing before/after retained trends

Do not use it as proof that a specific individual object moved or disappeared.

### The OQL/query surface is still growing

The current query engine is real and useful, but it is still narrower than a full MAT-style OQL environment.

Today it already supports:

- built-in fields
- retained instance-field projection and filtering on the query path
- `INSTANCEOF`

Still growing:

- richer predicates
- broader object-string rendering and explorer ergonomics
- deeper interactive browsing semantics

### Large dumps still pay the cost of an in-memory architecture

Mnemosyne has cleared its current large-dump validation gate, but it is not pretending to be a zero-memory-cost analyzer. Deep graph-backed commands still need meaningful headroom.

Best practice:

- triage with `parse`
- escalate to `analyze`, `leaks`, `query`, or `gc-path` only when needed

### Fix suggestions can occasionally look more optimistic than a fresh analyze run

`fix` is a separate remediation pipeline and can work from a narrower or different slice of context than the report you last looked at. In practice, that means you may occasionally see a fix suggestion even when a separate `analyze` run looked quiet or reported zero actionable leaks under different filters.

Treat fix output as a lead, not proof. When in doubt:

- rerun `leaks` and `explain` with the same filters you intend to use
- make sure the `leak-id` came from the same heap and filter set
- inspect provenance markers before turning a suggestion into code

## 6. Getting Help

If you are stuck after the checks above, the most useful next step is to file a precise issue rather than a generic "it failed" report.

Useful places:

- GitHub Issues: https://github.com/bballer03/mnemosyne/issues
- contribution guidance: [../CONTRIBUTING.md](../CONTRIBUTING.md)

Include these details in a bug report when you can:

- the exact command you ran
- whether you used a config file or environment overrides
- whether you were in `rules`, `stub`, or `provider` AI mode
- whether the failure happened in CLI or MCP usage
- the exact error text, including any `hint:` line or `error_details.code`
- whether the heap is real-world or synthetic, and its approximate size

If the issue is provider-related, include the provider type and whether the endpoint is cloud or local, but do not include raw secrets or sensitive heap content.