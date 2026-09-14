# Mnemosyne v0.4.0 Release Notes

> Release date: 2026-09-14
> Tag: v0.4.0
> Previous release: [v0.3.0](release-notes-v0.3.0.md)

Mnemosyne v0.4.0 is the first post-M16 desktop-capable release that also ships the UI-first MAT workbench loop: open a heap in the GUI, run bounded OQL, use policies/snapshots/flamegraphs from the workbench, and keep AI guidance advisory rather than a substitute for deterministic analysis.

## Highlights

- **UI workbench (M20):** Open a heap dump with an opaque `sourceId`, inspect detail panels, run Policies, open Snapshots (list/save/remove/open), and export Flamegraphs from the shared React UI / Tauri shell.
- **Portable install honesty (M21):** Frozen primary asset names, ditto-only macOS app zips, unsigned desktop `SHA256SUMS` + provenance, secret/structure probes, and **fail-closed** release CI gates. This release does **not** claim a proven native launch matrix from WSL.
- **Bounded MAT OQL (M22):** Multi-class `FROM`, 1–3 hop `OBJECTS`, `DISTINCT OBJECTS`, Unique Classes CLI column, and honest superclass hierarchy UI. Full MAT golden corpus closeout remains open.
- **AI-first assistants (M23):** `/assistant` Investigation session, workflow get/close/resume, and thin `chatSession` adapters. Rules mode stays the offline default; AI text stays visually separate from measured heap facts.
- **Post-v0.3.0 depth already on main:** M8–M19 reachability, snapshots, object diff, workflows, classloaders, UI parity, MAT backend parity, desktop packaging, MCP agent-loop tools, and guided continuity all ship in this tag.

## What's New in Detail

### Desktop and installability

Tagged `v0.4.0` builds attach Tauri desktop bundles alongside CLI archives (M16). Default artifacts are **unsigned** unless release signing secrets are present. Portable Windows zips still require WebView2; AppImage needs a WebKit runtime; macOS may hit Gatekeeper on unsigned builds. See [docs/evidence/m21-portable-installability.md](evidence/m21-portable-installability.md) and [SECURITY.md](../SECURITY.md#desktop-app-distribution-m16).

### UI workbench and AI guidance

The shared `ui/` frontend now carries the M20 workbench surfaces and M23 `/assistant` flow. Desktop bridges inject heap open/analysis, workflow lifecycle, and session chat adapters. **AI guidance does not replace histogram, dominator, inspector, OQL, or GC-path analysis.**

### OQL and MAT-facing CLI

`mnemosyne-cli query` and the shared query engine accept the M22 bounded expansions. `analyze --classloaders` prints Unique Classes. Named deferrals (`eval(...)`, unbounded hop depth, arbitrary subquery nesting) stay explicit.

## Breaking Changes

No intentional breaking CLI, MCP, or report-shape changes versus v0.3.0. New commands, optional fields, and UI routes are additive. Overview-compatible policy skip semantics and provenance labeling are unchanged.

## Known Limitations

- **Native GUI launch matrix not proven from WSL.** Packaged smoke and per-platform launch evidence remain host-blocked ([docs/evidence/m21-m22-remaining.md](evidence/m21-m22-remaining.md)).
- **MAT golden corpus / Sol closeouts still open** for M21–M23.
- **Homebrew SHA-256 values** in `HomebrewFormula/mnemosyne.rb` are placeholders until archives exist; update in a follow-up commit after the GitHub Release assets publish.
- **Signing secrets unset → unsigned desktop installers** (SmartScreen/Gatekeeper workarounds apply).
- **Comparative benchmarks remain the partial v0.3.0 WSL publication**; native-Linux reference-spec rerun is still pending.

## Upgrade Instructions

- **GitHub Release:** download CLI archives and desktop bundles from the `v0.4.0` release page.
- **Docker:** `docker pull ghcr.io/bballer03/mnemosyne:0.4.0`
- **Source:** `git checkout v0.4.0 && cargo build --release --workspace`
- **Homebrew:** after SHA follow-up, `brew upgrade mnemosyne` (formula version already points at `0.4.0` URLs).

## Links

- [CHANGELOG entry](../CHANGELOG.md#040---2026-09-14)
- [STATUS](../STATUS.md)
- [UI capability matrix](product/ui-capability-matrix.md)
- [M21 portable install evidence](evidence/m21-portable-installability.md)
- [M21/M22 remaining](evidence/m21-m22-remaining.md)
- [M23 guided investigation evidence](evidence/m23-guided-investigation.md)
- [Roadmap](roadmap.md)
