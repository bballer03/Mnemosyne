import { describe, expect, it } from "bun:test";
import { existsSync } from "node:fs";
import { join } from "node:path";

import { defaultBatchTimeoutMs, uiTestBatches } from "./run-tests-config";
import {
  classifyBatchResult,
  exceedsRssCeiling,
  formatBatchGateFailure,
  parseProcStatusPeakRssKiB,
} from "./run-tests-rss";
import { runUiTestBatch } from "./run-tests-runner";

describe("run-tests RSS gate", () => {
  it("prefers VmHWM over VmRSS when parsing /proc status", () => {
    const status = [
      "Name:\tbun",
      "VmRSS:\t  102400 kB",
      "VmHWM:\t  204800 kB",
    ].join("\n");

    expect(parseProcStatusPeakRssKiB(status)).toBe(204800);
  });

  it("falls back to VmRSS when VmHWM is absent", () => {
    const status = "VmRSS:\t  51200 kB\n";
    expect(parseProcStatusPeakRssKiB(status)).toBe(51200);
  });

  it("detects RSS above the documented ceiling", () => {
    expect(exceedsRssCeiling(513 * 1024, 512)).toBeTrue();
    expect(exceedsRssCeiling(512 * 1024, 512)).toBeFalse();
  });

  it("names the batch on timeout, signal, and RSS failures", () => {
    expect(
      formatBatchGateFailure({
        kind: "timeout",
        batchId: "workflow-card",
        batchFiles: ["src/features/workflow-landing/WorkflowCard.test.tsx"],
      }),
    ).toContain('batch "workflow-card" timed out');

    expect(
      formatBatchGateFailure({
        kind: "signal",
        batchId: "workflow-landing",
        batchFiles: ["src/features/workflow-landing/GuidedLanding.test.tsx"],
        signal: "SIGTERM",
      }),
    ).toContain('batch "workflow-landing" terminated by signal SIGTERM');

    expect(
      formatBatchGateFailure({
        kind: "rss-ceiling",
        batchId: "investigation-assistant",
        batchFiles: ["src/features/assistant/InvestigationAssistantPage.test.tsx"],
        rssCeilingMiB: 768,
        peakRssMiB: 900,
      }),
    ).toContain('batch "investigation-assistant" exceeded RSS ceiling');
  });

  it("classifies SIGTERM as a failed gate before exit-code success", () => {
    const failure = classifyBatchResult({
      batchId: "workflow-card",
      batchFiles: ["src/features/workflow-landing/WorkflowCard.test.tsx"],
      rssCeilingMiB: 384,
      peakRssKiB: 100 * 1024,
      rssMeasured: true,
      exitCode: 0,
      signal: "SIGTERM",
      timedOut: false,
    });

    expect(failure?.kind).toBe("signal");
  });

  it("classifies RSS ceiling breaches after a zero exit code", () => {
    const failure = classifyBatchResult({
      batchId: "heap-explorer-core",
      batchFiles: ["src/features/heap-explorer/HeapExplorerLayout.test.tsx"],
      rssCeilingMiB: 512,
      peakRssKiB: 600 * 1024,
      rssMeasured: true,
      exitCode: 0,
      signal: null,
      timedOut: false,
    });

    expect(failure?.kind).toBe("rss-ceiling");
  });
});

describe("run-tests batch config", () => {
  it("uses unique batch ids and existing test files", () => {
    const ids = new Set<string>();

    for (const batch of uiTestBatches) {
      expect(batch.id.length).toBeGreaterThan(0);
      expect(ids.has(batch.id)).toBeFalse();
      ids.add(batch.id);

      expect(batch.rssCeilingMiB).toBeGreaterThan(0);
      expect(batch.timeoutMs ?? defaultBatchTimeoutMs).toBe(defaultBatchTimeoutMs);

      for (const file of batch.files) {
        expect(existsSync(join(import.meta.dir, file))).toBeTrue();
      }
    }
  });

  it("keeps investigation-assistant on the highest documented ceiling", () => {
    const assistant = uiTestBatches.find((batch) => batch.id === "investigation-assistant");
    const maxCeiling = Math.max(...uiTestBatches.map((batch) => batch.rssCeilingMiB));
    expect(assistant?.rssCeilingMiB).toBe(maxCeiling);
  });
});

describe("runUiTestBatch harness", () => {
  it("reports RSS ceiling failures from polled peaks", async () => {
    const result = await runUiTestBatch(
      {
        id: "harness-probe",
        files: ["run-tests-harness-fixture.test.ts"],
        rssCeilingMiB: 1,
        timeoutMs: 30_000,
      },
      {
        inheritStdio: false,
        pollPeakRss: () => 64 * 1024,
      },
    );

    expect(result.failure?.kind).toBe("rss-ceiling");
    expect(result.failure?.batchId).toBe("harness-probe");
  });
});
