# Dependency currency inventory (registry-verified)

**Date:** 2026-09-15 (M32.A+B applied)
**Sources of truth:** [npmjs.com](https://www.npmjs.com) `dist-tags.latest`, [crates.io](https://crates.io) `max_stable_version`, Docker Hub `library/rust` tags, GitHub Releases for Actions.  
**Program home:** [UI → MAT maturity roadmap](../superpowers/specs/2026-09-15-ui-mat-maturity-roadmap-design.md) § **M32 Dependency & toolchain currency**  
**Open Dependabot majors (superseded by M32):** [#50](https://github.com/bballer03/Mnemosyne/pull/50) `@types/react` 19, [#51](https://github.com/bballer03/Mnemosyne/pull/51) `@vitejs/plugin-react` 6, [#52](https://github.com/bballer03/Mnemosyne/pull/52) `tailwindcss` 4.

> Rule: pin targets come **only** from the registry `latest` / max stable listed below at upgrade time. Re-query before each M32 slice lands — do not trust this table after the date above without a refresh.

---

## 1. UI (`ui/package.json`) — npm `latest`

| Package | Manifest today | Registry `latest` (2026-09-15) | Delta |
|---|---|---|---|
| `react` | ^19.3.0 | 19.3.0 | **current (M32.A)** |
| `react-dom` | ^19.3.0 | 19.3.0 | **current (M32.A)** |
| `@types/react` | ^19.3.0 | 19.3.0 | **current (M32.A)** — closes #50 |
| `@types/react-dom` | ^19.3.0 | 19.3.0 | **current (M32.A)** |
| `react-router-dom` | ^7.18.3 | 7.18.3 | **current (M32.A)** |
| `vite` | ^8.3.0 | 8.3.0 | **current (M32.A)** |
| `@vitejs/plugin-react` | ^6.1.1 | 6.1.1 | **current (M32.A)** — closes #51 |
| `tailwindcss` | 4.3.3 | 4.3.3 | **current (M32.B)** — closes #52 |
| `@tailwindcss/postcss` | 4.3.3 | 4.3.3 | **current (M32.B)** |
| `jsdom` | ^24.1.1 | **30.0.1** | major (historically OOM-correlated on bun) — **M32.C** |
| `@tanstack/react-table` | 9.2.4 | 9.2.4 | **current (M32.B)** |
| `@testing-library/jest-dom` | 7.0.1 | 7.0.1 | **current (M32.B)** |
| `typescript` | 7.0.2 | 7.0.2 | **current (M32.B)** |
| `@tanstack/react-query` | ^5.102.8 | 5.102.8 | current |
| `@tauri-apps/api` | ^2.11.1 | 2.11.1 | current |
| `clsx` | ^2.1.1 | 2.1.1 | current |
| `zustand` | ^5.0.15 | 5.0.15 | current |
| `@testing-library/react` | ^16.3.3 | 16.3.3 | current |
| `@testing-library/user-event` | ^14.6.7 | 14.6.7 | current |
| `autoprefixer` | ^10.6.0 | 10.6.0 | current |
| `postcss` | ^8.5.28 | 8.5.28 | current |
| `bun-types` | ^1.4.2 | 1.4.2 | current |

**Toolchain:** Bun GitHub latest = `bun-v1.4.2`. CI currently pins `bun-version: "1.2.5"` for UI-test memory safety — M32 must re-measure RSS before bumping CI Bun.

---

## 2. Rust workspace — crates.io `max_stable_version`

| Crate | Pin today | Latest stable | Notes |
|---|---|---|---|
| `clap` | 4.6 | **4.6.7** | current |
| `tokio` | 1.53 (workspace) / 1 (tauri) | **1.53.1** | current workspace pin |
| `toml` | 1.1.6 | **1.1.6** | current |
| `reqwest` | 0.13.5 | **0.13.5** | current |
| `dirs` | 7 (workspace and tauri) | **7.0.0** | current |
| `comfy-table` | 8 | **8.0.0** | current |
| `serde_yaml` | 0.9 | 0.9.34+**deprecated** | **Exception (2026-09-15, owner: M32):** retain 0.9 for this slice; replacement migration requires separate compatibility work. |
| `anyhow` | 1.0 | 1.0.104 | caret already allows |
| `console` | 0.16 | 0.16.6 | caret |
| `indicatif` | 0.18 | 0.18.6 | caret |
| `petgraph` | 0.8 | 0.8.3 | caret |
| `thiserror` | 2.0 | 2.0.20 | caret |
| `inferno` | 0.12 | 0.12.8 | caret |
| `criterion` | 0.8 | 0.8.2 | caret |
| `tauri` | 2 | 2.11.5 | caret on major |
| `tauri-build` | 2 | 2.6.3 | caret |
| `tauri-plugin-dialog` | 2 | 2.7.3 | caret |
| `uuid` | 1.26.1 | 1.26.1 | current |
| `xxhash-rust` | 0.8 | 0.8.18 | caret |

---

## 3. Docker / Actions

| Surface | Today | Registry / GitHub latest |
|---|---|---|
| `Dockerfile` rust image | `rust:1.97-bookworm` | **`rust:1.98.1-bookworm`** (Hub) |
| `actions/checkout` | SHA for v7.0.1 | v7.0.1 (current) |
| `oven-sh/setup-bun` | SHA for v2.2.0 | v2.2.0 (current) |
| `docker/setup-buildx-action` | (check pin in workflow) | **v4.3.0** |
| `docker/login-action` | | **v4.6.0** |
| `docker/build-push-action` | | **v7.3.0** |
| `docker/metadata-action` | | **v6.2.0** |
| `softprops/action-gh-release` | | **v3.0.3** |

Re-resolve Action SHAs from the tagged release commit when bumping.

---

## 4. Upgrade policy (M32)

1. **Re-query registry** at slice start; update this file’s date + numbers.  
2. Prefer **one ecosystem major cluster per PR** (UI React/Vite/RR, UI Tailwind, UI jsdom+test runner, Cargo toml/reqwest/dirs, Docker/Actions).  
3. **CI is the validator** for UI suites (no full local bun suite on constrained WSL). Measure peak RSS before raising CI Bun past 1.2.5.  
4. Close Dependabot #50–#52 only after the consolidated UI majors land (or supersede with comments).  
5. Never treat `deprecated` crates as “latest forever” — schedule migration.  
6. After each green cluster: sync `STATUS.md`, `CHANGELOG.md` Unreleased, and this inventory.

---

## 5. Acceptance for M32 complete

- Every direct dependency in `ui/package.json`, workspace/`cli`/`core`/`tauri` manifests, Dockerfile rust tag, and Actions pins matches registry `latest` / max stable **or** has an explicit, dated exception in this file with owner + reason.  
- UI + Rust + Docker CI green on the upgrade branch.  
- Dependabot majors #50–#52 closed or superseded.  
- No silent jsdom/Bun memory regression (RSS evidence attached).
