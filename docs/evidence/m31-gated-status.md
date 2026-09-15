# M31+ gated follow-ons — status

**Date:** 2026-09-16 (Wave 2 Windows-first update)
**Plan stub:** [docs/superpowers/plans/2026-09-15-m31-gated-follow-ons.md](../superpowers/plans/2026-09-15-m31-gated-follow-ons.md)
**Program design:** [docs/superpowers/specs/2026-09-15-eclipse-mat-equivalent-program-design.md](../superpowers/specs/2026-09-15-eclipse-mat-equivalent-program-design.md)

Product owner opened selected gates for the **credible MAT-equivalent** program. macOS/Linux launch remains deferred.

## Gate assessment

| Slice | Gate status | Notes |
| --- | --- | --- |
| **31.A** Desktop OS integration | **FOCUSED CONTRACT MET (Windows-first)** | Installer association metadata, portable Open With helper, native File menu, and opaque recent heaps implemented; Windows build/menu/helper proven. Installed Explorer double-click **NOT PROVEN**; macOS/Linux deferred |
| **31.B** MAT golden / equivalence | **IN PROGRESS** | Three `mnemosyne-golden` synthetic cases run in CI; `mat-referenced` = 0, Eclipse MAT cross-check **NOT PROVEN** |
| **31.C** Perspectives / layout presets | **FOCUSED CONTRACT MET** | Four named fixed presets with session-scoped workspace persistence; packaged GUI **NOT PROVEN** |
| **31.D** Plugins / live JVM | **NOT MET** | Still requires security/resource model |
| **31.E** Signing / updater / Cask | **NOT MET** | Still requires secrets + product decision |

## Active program waves

See design §5: Wave 0 Open-heap + live AI → Wave 1 golden + perspectives + UI gaps → Wave 2 Windows OS integration.

Wave evidence: [31.A Windows OS integration](m31a-windows-os-integration.md), [31.B golden harness](m31b-mat-golden.md), [31.C perspectives](m31c-perspectives.md).
