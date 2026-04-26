## Description

Describe the change and the problem it solves.

## Type of Change

- [ ] Bug fix
- [ ] New feature
- [ ] Breaking change
- [ ] Refactor
- [ ] Test coverage
- [ ] Documentation only

## Design Reference

<!-- Required for non-trivial changes per the pre-coding design gate (.github/copilot-instructions.md). -->
<!-- Link the design doc under docs/design/ or the spec under docs/superpowers/specs/. Write `N/A` for trivial changes. -->

`docs/design/<file>.md` or `docs/superpowers/specs/<file>.md` or N/A

## Checklist

- [ ] `cargo test --workspace` passes locally
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` is clean locally
- [ ] `cargo fmt --all` has been applied
- [ ] Tests were added or updated for new behavior (or a documented reason why not)
- [ ] Failing test was written first per [tdd-cycle](../.github/skills/tdd-cycle/SKILL.md) (or N/A for non-behavior changes)
- [ ] Documentation has been updated (README, docs/, CHANGELOG.md as applicable)
- [ ] CLI / MCP / config / report contracts remain aligned across code and docs
- [ ] No sensitive heap data, secrets, or credentials in logs or test fixtures

## Related Issues

Closes #