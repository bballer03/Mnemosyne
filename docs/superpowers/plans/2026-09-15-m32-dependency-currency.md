# M32 Dependency & Toolchain Currency — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring UI, Cargo, Docker, and Actions pins to registry-verified `latest` / max stable; close Dependabot #50–#52; keep CI green with RSS evidence for jsdom/Bun.

**Architecture:** One PR cluster per ecosystem major; re-query registries at slice start; validate via GitHub CI (avoid full local bun/cargo suites on WSL).

**Tech Stack:** npm (ui/), Cargo workspace + tauri, Docker Hub `rust`, GitHub Actions.

**Spec:** [2026-09-15-ui-mat-maturity-roadmap-design.md](../specs/2026-09-15-ui-mat-maturity-roadmap-design.md) § M32  
**Inventory:** [dependency-currency-inventory.md](../../product/dependency-currency-inventory.md)

## Global Constraints

- Targets from registry only (npm `latest`, crates.io `max_stable_version`, Docker Hub, GitHub Releases).
- Do not run full `bun run test` or `cargo test --workspace` on constrained WSL — push and use CI.
- Preserve lean-open defaults; no field-data-by-default.
- Close or supersede Dependabot PRs #50–#52 after consolidated upgrades.
- Update inventory date + STATUS/CHANGELOG on closeout.

---

## File map

| Area | Files |
|---|---|
| UI deps | `ui/package.json`, `ui/bun.lock`, Tailwind/PostCSS config, Vite config, any React 19 / RR7 API fixes under `ui/src/` |
| Cargo | `Cargo.toml`, `Cargo.lock`, `cli/Cargo.toml`, `core/Cargo.toml`, `tauri/Cargo.toml`, `tauri/Cargo.lock`, `tauri/session-ops/Cargo.toml` |
| Docker/CI | `Dockerfile`, `.github/workflows/ci.yml`, `.github/workflows/release.yml` |
| Docs | `docs/product/dependency-currency-inventory.md`, `STATUS.md`, `CHANGELOG.md` |

---

### Task 32.0 — Branch + docs lock

- [ ] Ensure branch `feature/v0.5.0-mat-maturity` from `main`
- [ ] Commit approved roadmap, inventory, M24 open-another amendment, Homebrew SHA for v0.4.3, STATUS/roadmap sync
- [ ] Push branch

### Task 32.E — Docker + Actions (low blast radius)

- [ ] Re-query Hub for latest `*-bookworm` rust tag; set `Dockerfile` to `rust:1.98.1-bookworm` (or newer if registry moved)
- [ ] Re-query Action release tags; update SHA pins in `ci.yml` / `release.yml` where behind
- [ ] Commit `build(deps): rust 1.98.1-bookworm + actions pin refresh`
- [ ] Confirm CI Docker job green

### Task 32.D — Cargo majors

- [ ] Re-query crates.io; bump `toml`→1.x, `reqwest`→0.13, `dirs`→7 (unify tauri), `comfy-table`→8, workspace `clap`/`tokio` pins
- [ ] Fix compile breaks; leave `serde_yaml` with dated exception OR migrate if low-cost
- [ ] `cargo update` lockfiles; commit; CI Build & Test green

### Task 32.A — UI framework majors (React 19 / RR7 / Vite 8)

- [ ] Re-query npm; bump react, react-dom, @types/*, react-router-dom, vite, @vitejs/plugin-react
- [ ] Fix Router / types breakages; commit
- [ ] CI UI green

### Task 32.B — Tailwind 4 + table 9 + jest-dom 7 + TS 7

- [ ] Migrate Tailwind 3→4 (PostCSS plugin / `@import "tailwindcss"`)
- [ ] Bump react-table, jest-dom, typescript; fix types
- [ ] Commit; CI UI green

### Task 32.C — jsdom 30 + Bun CI pin

- [x] Attempted jsdom 30.0.1 — Bun hung >2min after focused tests; **deferred** with dated inventory exception (keep ^24.1.1); commit `2b4de0e`
- [ ] Only raise `bun-version` above 1.2.5 with measured RSS
- [x] Exception documented (do not force jsdom 30)

### Task 32.F — Closeout

- [x] Comment+close Dependabot #50–#52 as superseded
- [x] Inventory refreshed (jsdom exception dated)
- [ ] Merge PR #93 when CI green / remaining M24 slices land

---

## Execution notes

- Prefer multiple PRs if one cluster fails: `m32-docker`, `m32-cargo`, `m32-ui-framework`, etc. under same epic branch or stacked PRs.
- After M32 merges (or UI CI green on branch), start M24 plan immediately.
