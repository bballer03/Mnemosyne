import { readFileSync } from "node:fs";

export type BatchGateFailureKind = "timeout" | "signal" | "exit-code" | "rss-ceiling" | "spawn-error";

export type BatchGateFailure = {
  kind: BatchGateFailureKind;
  batchId: string;
  batchFiles: readonly string[];
  rssCeilingMiB?: number;
  peakRssMiB?: number;
  exitCode?: number | null;
  signal?: NodeJS.Signals | null;
  detail?: string;
};

export type BatchRunMetrics = {
  batchId: string;
  peakRssKiB: number;
  peakRssMiB: number;
  rssCeilingMiB: number;
  exitCode: number | null;
  signal: NodeJS.Signals | null;
  timedOut: boolean;
  rssMeasured: boolean;
};

/**
 * Parse peak resident set from Linux `/proc/<pid>/status`.
 * Prefers VmHWM (high water mark); falls back to instantaneous VmRSS.
 */
export function parseProcStatusPeakRssKiB(status: string): number {
  const hwm = status.match(/^VmHWM:\s+(\d+)/m);
  if (hwm) {
    return Number(hwm[1]);
  }

  const rss = status.match(/^VmRSS:\s+(\d+)/m);
  return rss ? Number(rss[1]) : 0;
}

export function readProcessPeakRssKiB(pid: number): number | undefined {
  try {
    const status = readFileSync(`/proc/${pid}/status`, "utf8");
    const peak = parseProcStatusPeakRssKiB(status);
    return peak > 0 ? peak : undefined;
  } catch {
    return undefined;
  }
}

export function kiBToMiB(kib: number): number {
  return kib / 1024;
}

export function exceedsRssCeiling(peakRssKiB: number, ceilingMiB: number): boolean {
  return kiBToMiB(peakRssKiB) > ceilingMiB;
}

export function formatBatchGateFailure(failure: BatchGateFailure): string {
  const fileList = failure.batchFiles.join("\n  ");
  switch (failure.kind) {
    case "timeout":
      return [
        `UI test batch "${failure.batchId}" timed out (likely hang/OOM).`,
        failure.detail ?? "The runner killed the batch after the per-batch wall-clock limit.",
        "Files:",
        `  ${fileList}`,
      ].join("\n");
    case "signal":
      return [
        `UI test batch "${failure.batchId}" terminated by signal ${failure.signal ?? "unknown"}.`,
        failure.detail ?? "Treat runner SIGTERM/cancel as a failed gate for this batch.",
        "Files:",
        `  ${fileList}`,
      ].join("\n");
    case "rss-ceiling":
      return [
        `UI test batch "${failure.batchId}" exceeded RSS ceiling.`,
        `Peak RSS: ${failure.peakRssMiB?.toFixed(1) ?? "?"} MiB; ceiling: ${failure.rssCeilingMiB ?? "?"} MiB.`,
        failure.detail ?? "Split the batch or fix the leak before raising the ceiling.",
        "Files:",
        `  ${fileList}`,
      ].join("\n");
    case "exit-code":
      return [
        `UI test batch "${failure.batchId}" exited with code ${failure.exitCode ?? "unknown"}.`,
        "Files:",
        `  ${fileList}`,
      ].join("\n");
    case "spawn-error":
      return [
        `UI test batch "${failure.batchId}" failed to start.`,
        failure.detail ?? "Unknown spawn error.",
        "Files:",
        `  ${fileList}`,
      ].join("\n");
    default:
      return `UI test batch "${failure.batchId}" failed.`;
  }
}

export function classifyBatchResult(input: {
  batchId: string;
  batchFiles: readonly string[];
  rssCeilingMiB: number;
  peakRssKiB: number;
  rssMeasured: boolean;
  exitCode: number | null;
  signal: NodeJS.Signals | null;
  timedOut: boolean;
}): BatchGateFailure | undefined {
  if (input.timedOut) {
    return {
      kind: "timeout",
      batchId: input.batchId,
      batchFiles: input.batchFiles,
    };
  }

  if (input.signal) {
    return {
      kind: "signal",
      batchId: input.batchId,
      batchFiles: input.batchFiles,
      signal: input.signal,
    };
  }

  if (input.exitCode !== 0) {
    return {
      kind: "exit-code",
      batchId: input.batchId,
      batchFiles: input.batchFiles,
      exitCode: input.exitCode,
    };
  }

  if (input.rssMeasured && exceedsRssCeiling(input.peakRssKiB, input.rssCeilingMiB)) {
    return {
      kind: "rss-ceiling",
      batchId: input.batchId,
      batchFiles: input.batchFiles,
      rssCeilingMiB: input.rssCeilingMiB,
      peakRssMiB: kiBToMiB(input.peakRssKiB),
    };
  }

  return undefined;
}
