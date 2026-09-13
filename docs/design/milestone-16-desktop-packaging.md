# Milestone 16 — Desktop Packaging & Distribution

> **Status:** 🔲 Pending — design authored 2026-08-20, awaiting Implementation Agent pickup of Slice 16.A.
> **Owner (design):** Design Consulting Agent (this pass, run inline by the orchestrating session per user directive — no human gate)
> **Owner (implementation):** Implementation Agent (per slice, subagent-driven)
> **Parent:** [docs/roadmap.md §5](../roadmap.md) — M16
> **Predecessors:** M6 (Tauri scaffold, shipped). M14 (UI backend-parity & AI-native redesign) — this milestone bundles whatever GUI M14 ships; sequenced after it, though the release-pipeline plumbing in §4 can be built and tested against today's UI in the meantime.
> **Last updated:** 2026-08-20

---

## 1. Status / metadata

| Field | Value |
|---|---|
| Milestone | M16 |
| Type | Adoption (packaging/distribution, no new analysis capability) |
| Touched crates/files | `tauri/`, `.github/workflows/release.yml`, `HomebrewFormula/` (if the desktop app should also be Homebrew-installable, TBD in design), repo root release docs. |
| Test bar | A built installer actually launches, loads a real heap dump, and completes an analysis on each target platform this milestone claims to support — not just "the build succeeded." |
| Environment constraint | **Flagged now, per §6 R1:** this executing sandbox is Windows/MSYS2 with no macOS or Linux code-signing credentials available (same class of constraint that blocked M12). Slices requiring platform-specific signing infrastructure this environment cannot provide must be scoped as "produce the build, document the signing step as a manual/CI-secret-gated follow-up" rather than silently claimed as done. |

## 2. Objective

After M16, a user can download one installer for their platform from GitHub Releases (alongside the existing 5-target CLI archives), run it, and get the full M14 GUI without installing Rust, Node/Bun, or building from source — the same adoption path Eclipse MAT itself offers today.

## 3. Context

### 3.1 What Mnemosyne ships today (inspected)

- [tauri/Cargo.toml](../../tauri/Cargo.toml) — `mnemosyne-desktop` binary, Tauri **v2** (`tauri = { version = "2", features = ["devtools"] }`), already depends on `mnemosyne-core` directly (no IPC/subprocess boundary — the desktop app links the analysis engine in-process).
- [tauri/tauri.conf.json](../../tauri/tauri.conf.json) — `bundle.active: true`, `bundle.targets: "all"` (Tauri's bundler already configured to produce every platform-native format it knows how to build: `.msi`/`.exe` on Windows, `.dmg`/`.app` on macOS, `.deb`/`.AppImage`/`.rpm` on Linux). Icons already present (`32x32.png`, `128x128.png`, `.icns`, `.ico`). `beforeBuildCommand: "cd ../ui && bun run build"` — the Tauri build already wires in the `ui/` frontend build step, meaning **M14's UI work needs zero packaging-side changes to be included** once it lands; this milestone doesn't have to "hook up" the frontend, that plumbing already exists.
- [.github/workflows/release.yml](../../.github/workflows/release.yml) — existing 5-target CLI release matrix (`x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, `x86_64-apple-darwin`, `aarch64-apple-darwin`, `x86_64-pc-windows-msvc`), tag-triggered, publishes to GitHub Releases + GHCR + (separately) Homebrew formula bump. **No Tauri/desktop build step exists in this workflow today** — this is the actual gap M16 closes, not the Tauri scaffold itself (which is already release-bundle-configured).
- `tauri/src/commands.rs` — native commands for `load_heap`, `unload_heap`, `query_heap`, `get_references`, `get_referrers`, `explain_leak`, `find_gc_path`, `map_to_code`, `propose_fix`. M9/M10/M11/M13's newer capabilities have no native-command equivalent yet — in scope for this milestone only where M14's UI genuinely needs one to function in the desktop shell (see §4 "Out").

### 3.2 What MAT does

Eclipse MAT ships as a standalone Eclipse RCP application: a per-platform zip/dmg/exe bundling a full Eclipse runtime + JRE, downloadable from eclipse.org, no separate JVM install required by the user (MAT bundles its own). M16's target user experience is the same shape — no separate Rust/Bun toolchain required — achieved via Tauri's single-binary-plus-webview model instead of a bundled runtime, which is actually a lighter footprint than MAT's Eclipse-RCP-plus-JRE bundle (a real differentiator worth preserving, not just parity).

## 4. Scope

In:

1. **CI release-pipeline integration:** extend `.github/workflows/release.yml` (or add a sibling `desktop-release.yml` — implementation's call based on how disruptive extending the existing matrix job proves to be) with a Tauri build step per platform, using `tauri-action` (the standard GitHub Action for this, check its current major version before pinning) or a manual `cargo tauri build` invocation matching however the existing CLI matrix job is structured. Produces the same "all" bundle targets `tauri.conf.json` already declares.
2. **Attach desktop installers to the same tagged GitHub Release** the CLI archives already publish to — one release, both the CLI binaries and the desktop installers, matching how a user browsing a Mnemosyne release today would expect to find everything in one place.
3. **Code-signing — scoped honestly per platform (§6 R1):**
   - **Windows:** Tauri supports Authenticode signing via a certificate + `signtool`; requires a code-signing certificate this environment does not have. Scope this slice as: wire the CI step to sign **if** the relevant GitHub Actions secrets are present, ship unsigned with a clear `SECURITY.md`/release-notes caveat if they are not — do not block the whole milestone on acquiring a certificate, which is an organizational/purchasing decision outside an autonomous coding session's authority.
   - **macOS:** Apple notarization requires an Apple Developer account + credentials, same "if secrets present, sign; otherwise document unsigned" scoping. Unsigned macOS apps trigger Gatekeeper warnings — document the user-facing workaround (right-click → Open) in the user guide rather than silently shipping a broken-feeling first-run experience.
   - **Linux:** no equivalent OS-level signing gate exists for `.deb`/`.AppImage`/`.rpm` the way Windows/macOS gate unsigned binaries — ship unsigned, no special handling needed.
4. **Auto-update:** explicit decision needed at implementation time, not assumed — Tauri has a built-in updater plugin, but wiring it requires a signing-key infrastructure decision (update artifacts need to be signed for the updater to trust them) that overlaps with item 3's constraints. **Default scope for v1: document the decision to skip auto-update** (users re-download from GitHub Releases for new versions, same as the CLI today) unless implementation finds this materially cheaper than expected once the signing-key question from item 3 is resolved.
5. **User-facing installation docs:** extend `README.md`'s existing "Installation" section (which already documents CLI install paths: release binary, cargo, Homebrew, Docker, source) with a "Desktop app" subsection — download link pattern, per-platform first-run notes (including the Gatekeeper workaround from item 3 if macOS ships unsigned).
6. **Homebrew Cask (optional, decide at implementation time):** the existing `HomebrewFormula/mnemosyne.rb` is a **Formula** (CLI binary). A desktop `.app` distributed via Homebrew would need a separate **Cask** — evaluate whether this is worth the maintenance surface for a v1 desktop release or whether "download from GitHub Releases" is sufficient distribution for now (lean toward the latter unless implementation finds it trivial — avoid scope creep into a second packaging channel this milestone didn't originally need).

Out:

- **New Tauri native commands** beyond what a specific M14 UI slice genuinely can't function without (per M14's own §4 "Out" scope boundary — this milestone does not do a blanket audit of every M9/M10/M11/M13 capability's desktop-native-command equivalent).
- **Acquiring actual code-signing certificates/accounts.** This is an organizational decision (cost, entity registration) outside what an autonomous coding session can execute — the deliverable here is the CI wiring that *uses* secrets if present, not obtaining the secrets themselves.
- **Auto-update infrastructure**, unless item 4's implementation-time evaluation finds it cheap given whatever signing-key decision item 3 lands on.
- **A second packaging channel** (Homebrew Cask, winget, Flatpak, Snap) beyond GitHub Releases, unless item 6's evaluation specifically justifies one.

## 5. Sub-slice plan

All slices end with a real built-and-launched installer verification on at least one platform this environment can actually test (Windows, given the executing sandbox — cross-platform builds may need to be verified via CI logs/artifacts rather than local launch if this environment can't run macOS/Linux binaries directly; be explicit about which platforms were genuinely launch-tested vs. build-verified-only in each slice's report).

### Slice 16.A — CI pipeline: build + attach unsigned installers

- **Scope:** Items 1–2. Get a real installer artifact attached to a tagged release, unsigned, on all three OS families, before touching signing at all — prove the pipeline works end-to-end first.
- **Files owned:** `.github/workflows/release.yml` (or new sibling workflow file), possibly `tauri/tauri.conf.json` if bundle config needs adjustment for CI (verify locally first with `cargo tauri build` if the toolchain is available in this environment, or document that it could only be verified via workflow-syntax review, not a real run, if not).
- **Validation gates:** a real (or dry-run/workflow-lint-verified, documented which) release produces installer artifacts for Windows at minimum (this environment's own platform — highest-confidence verification), with macOS/Linux artifacts at least build-attempted in CI even if this session can't launch-test them directly.
- **Target size:** CI config + small `tauri.conf.json` adjustments if needed.

### Slice 16.B — Code-signing wiring (conditional-on-secrets) + auto-update decision

- **Scope:** Item 3 (signing wired to check for secrets, unsigned fallback documented) and item 4 (explicit auto-update decision recorded, default: skip for v1).
- **Files owned:** `.github/workflows/release.yml`, `SECURITY.md` or `README.md` (signing/Gatekeeper caveat documentation).
- **Validation gates:** the signing step visibly no-ops (not silently, a clear CI log line) when secrets are absent, so a future maintainer adding real certificates knows exactly where to plug them in; the auto-update decision is written down, not left implicit.
- **Target size:** CI config + documentation.

### Slice 16.C — Installation docs + optional Homebrew Cask evaluation

- **Scope:** Items 5–6.
- **Files owned:** `README.md`, possibly new `HomebrewFormula/mnemosyne-desktop.rb` (Cask) if item 6's evaluation favors it.
- **Validation gates:** documentation accurately describes the actual shipped (possibly unsigned) experience per platform — no claiming a signed/notarized experience that Slice 16.B didn't actually achieve.
- **Target size:** Documentation + optional Cask file.

### Slice 16.D — Documentation sync

- **Scope:** `docs/roadmap.md` (mark M16 shipped or partially-shipped, being honest about which platforms got real signing vs. documented-unsigned), `STATUS.md`, `CHANGELOG.md`, design doc closeout.
- **Files owned:** Documentation only.
- **Validation gates:** Matches this session's established doc-sync precedent; explicit about signing status per platform, not glossed over.
- **Target size:** Documentation-only.

## 6. Risks and mitigations

| # | Risk | Mitigation |
|---|---|---|
| R1 | This executing environment cannot obtain or test real code-signing credentials (Windows Authenticode cert, Apple Developer account) | §4 item 3's binding scope: CI wiring is conditional-on-secrets, ships unsigned with documented user-facing caveats otherwise. Same "flag the environment constraint before claiming the milestone done" discipline M12 already established when it turned out fully blocked — this milestone is partially blocked on the same class of external-credential problem, not fully. |
| R2 | Unsigned installers trigger OS security warnings (SmartScreen on Windows, Gatekeeper on macOS) that read as "broken" to a new user | Document the exact click-through/right-click workaround in the README's Desktop app section (item 5) — same honesty-over-polish instinct as this project's `ProvenanceKind` contract elsewhere. |
| R3 | Cross-platform build verification is asymmetric — this session can genuinely launch-test Windows builds but only CI-log-verify macOS/Linux ones | Each slice's own report must be explicit about which platforms were actually launched vs. only build-verified — no claiming uniform confidence across platforms this session structurally cannot equally test. |
| R4 | Auto-update half-implemented (update check wired but signing/trust chain incomplete) becomes worse than no auto-update at all | §4 item 4's default scope is explicitly "skip for v1" unless implementation finds the full chain cheap — no partial/broken auto-update ships. |

## 7. Cross-references

- Parent: [docs/roadmap.md §5](../roadmap.md) — M16 backlog entry.
- Depends on: [milestone-14-ui-parity-ai-native.md](milestone-14-ui-parity-ai-native.md) (bundles whatever GUI M14 ships).
- Existing scaffold: [tauri/Cargo.toml](../../tauri/Cargo.toml), [tauri/tauri.conf.json](../../tauri/tauri.conf.json), [tauri/src/commands.rs](../../tauri/src/commands.rs).
- Existing release pipeline (structural template): [.github/workflows/release.yml](../../.github/workflows/release.yml).
- Precedent for honestly flagging an environment constraint rather than forcing a milestone: M12 (blocked entirely on missing native-Linux + Eclipse MAT reference hardware).

## 8. Implementation readiness verdict

**READY, with an environment caveat surfaced up front (not discovered mid-slice):** this executing sandbox has no macOS/Linux/Windows code-signing credentials, matching M12's earlier fully-blocked precedent but here only partially — Slice 16.A (unsigned pipeline) is fully executable in this environment; Slice 16.B's signing half is executable only as conditional CI wiring, not as an actual signed artifact, until real credentials are supplied by the user outside this session. Slice 16.A first. 16.B and 16.C are independent of each other once 16.A lands and may run in **separate isolated git worktrees**. 16.D is gated behind 16.A–16.C.
