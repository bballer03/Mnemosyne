# Diff Object Baselines

This slice captures local, indicative numbers for Milestone 8-1 Slice 8-1.G against the budget in [docs/design/milestone-8-1-object-level-diff.md](../design/milestone-8-1-object-level-diff.md) §12.

These numbers are not from the standard Mnemosyne benchmark host. They are local developer-host measurements and should be treated as directional only. CI should compile the bench via `cargo bench --no-run --workspace`; authoritative perf tracking belongs on the standard benchmark host.

The current synthetic pair is a pure-Rust stand-in generated inside the bench/test harnesses. It exercises multiple class hierarchies and roughly 10K retained objects, but it is still smaller than the official `medium` fixture described in the design doc.

Measurement host for this run:

- OS: Microsoft Windows 11 Home Single Language
- CPU: AMD Ryzen 9 5900HX with Radeon Graphics
- Criterion mode: `cargo bench -p mnemosyne-core --bench diff_object -- --quick`
- Memory sampling: benchmark-process working set sampled on the local host; this is an RSS-adjacent approximation, not the standard Mnemosyne benchmark-host RSS method

| metric | class diff (today) | object diff target | tolerance | measured value (this run) |
| --- | --- | --- | --- | --- |
| wall-clock cold | `class_diff_baseline` | `<= 2.0x` class diff | `+25%` allowed before red | class `20.91 ms`; object `class+retained 23.60 ms` (`1.13x`); object `class+dominator 20.57 ms` (`0.98x`) |
| peak RSS | local baseline | `<= 1.6x` class diff | `+20%` allowed before red | sampled working set: class `12.00 MiB`; object `class+dominator 9.72 MiB` |
| output size (JSON @ --top 50) | n/a | `<= 5 MB` | hard cap | object `class+dominator 5,166 bytes` |

Notes:

- `object_diff_full_fingerprint` was not added in this slice. The synthetic stand-in exercises the default `class+dominator` path and the regression baseline; full-fingerprint remains opt-in and higher-cost.
- `fixtures-real` is opt-in and currently runs the larger synthetic stand-in because the repository does not yet include a committed real diff pair.
- The working-set figures were sampled from the compiled Criterion bench process on Windows, so they should be read as host-local approximations rather than authoritative cross-host RSS numbers.