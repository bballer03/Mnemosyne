import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";

import {
  defaultBatchTimeoutMs,
  type UiTestBatch,
  uiTestBatches,
} from "./run-tests-config";
import {
  classifyBatchResult,
  formatBatchGateFailure,
  readProcessPeakRssKiB,
  type BatchRunMetrics,
} from "./run-tests-rss";

const pollIntervalMs = 50;
/** After wall-clock timeout, escalate SIGTERM → SIGKILL so hung Bun/jsdom cannot pin WSL RAM. */
export const batchKillEscalationMs = 5_000;

export type RunBatchOptions = {
  bunExecutable?: string;
  inheritStdio?: boolean;
  pollPeakRss?: (pid: number) => number | undefined;
  killEscalationMs?: number;
};

function defaultPollPeakRss(pid: number): number | undefined {
  return readProcessPeakRssKiB(pid);
}

export async function runUiTestBatch(
  batch: UiTestBatch,
  options: RunBatchOptions = {},
): Promise<{ metrics: BatchRunMetrics; failure?: ReturnType<typeof classifyBatchResult> }> {
  const bunExecutable = options.bunExecutable ?? process.execPath;
  const inheritStdio = options.inheritStdio ?? true;
  const pollPeakRss = options.pollPeakRss ?? defaultPollPeakRss;
  const timeoutMs = batch.timeoutMs ?? defaultBatchTimeoutMs;
  const killEscalationMs = options.killEscalationMs ?? batchKillEscalationMs;

  let child: ChildProcessWithoutNullStreams | undefined;
  let timedOut = false;
  let peakRssKiB = 0;
  let rssSampled = false;

  const runPromise = new Promise<{ exitCode: number | null; signal: NodeJS.Signals | null }>(
    (resolve, reject) => {
      child = spawn(bunExecutable, ["test", ...batch.files, "--max-concurrency=1"], {
        stdio: inheritStdio ? "inherit" : "pipe",
      });

      const sampleChildRss = () => {
        if (!child?.pid) {
          return;
        }
        const sample = pollPeakRss(child.pid);
        if (sample !== undefined) {
          rssSampled = true;
          peakRssKiB = Math.max(peakRssKiB, sample);
        }
      };

      sampleChildRss();

      let killEscalationTimer: ReturnType<typeof setTimeout> | undefined;
      const timer = setTimeout(() => {
        timedOut = true;
        child?.kill("SIGTERM");
        killEscalationTimer = setTimeout(() => {
          // Bun/jsdom regex hangs often ignore SIGTERM; force-reclaim WSL memory.
          child?.kill("SIGKILL");
        }, killEscalationMs);
      }, timeoutMs);

      const pollTimer = setInterval(sampleChildRss, pollIntervalMs);

      child.on("error", (error) => {
        clearTimeout(timer);
        if (killEscalationTimer) {
          clearTimeout(killEscalationTimer);
        }
        clearInterval(pollTimer);
        reject(error);
      });

      child.on("close", (exitCode, signal) => {
        clearTimeout(timer);
        if (killEscalationTimer) {
          clearTimeout(killEscalationTimer);
        }
        clearInterval(pollTimer);
        sampleChildRss();
        resolve({ exitCode, signal });
      });
    },
  );

  let exitCode: number | null = null;
  let signal: NodeJS.Signals | null = null;

  try {
    ({ exitCode, signal } = await runPromise);
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    return {
      metrics: {
        batchId: batch.id,
        peakRssKiB,
        peakRssMiB: peakRssKiB / 1024,
        rssCeilingMiB: batch.rssCeilingMiB,
        exitCode,
        signal,
        timedOut,
        rssMeasured: rssSampled,
      },
      failure: {
        kind: "spawn-error",
        batchId: batch.id,
        batchFiles: batch.files,
        detail,
      },
    };
  }

  const metrics: BatchRunMetrics = {
    batchId: batch.id,
    peakRssKiB,
    peakRssMiB: peakRssKiB / 1024,
    rssCeilingMiB: batch.rssCeilingMiB,
    exitCode,
    signal,
    timedOut,
    rssMeasured: rssSampled,
  };

  const failure = classifyBatchResult({
    batchId: batch.id,
    batchFiles: batch.files,
    rssCeilingMiB: batch.rssCeilingMiB,
    peakRssKiB,
    rssMeasured: rssSampled,
    exitCode,
    signal,
    timedOut,
  });

  return { metrics, failure };
}

export async function runAllUiTestBatches(
  batches: readonly UiTestBatch[] = uiTestBatches,
  options: RunBatchOptions = {},
): Promise<{ metrics: BatchRunMetrics[]; failure?: ReturnType<typeof classifyBatchResult> }> {
  const metrics: BatchRunMetrics[] = [];

  for (const batch of batches) {
    const result = await runUiTestBatch(batch, options);
    metrics.push(result.metrics);

    if (result.metrics.rssMeasured) {
      console.error(
        `[ui-test-rss] batch=${batch.id} peak=${result.metrics.peakRssMiB.toFixed(1)} MiB ceiling=${batch.rssCeilingMiB} MiB`,
      );
    } else {
      console.error(
        `[ui-test-rss] batch=${batch.id} peak=unavailable (non-Linux /proc); RSS ceiling not enforced for this batch`,
      );
    }

    if (result.failure) {
      return { metrics, failure: result.failure };
    }
  }

  return { metrics };
}

export function exitOnBatchFailure(
  failure: NonNullable<Awaited<ReturnType<typeof runAllUiTestBatches>>["failure"]>,
): never {
  console.error(formatBatchGateFailure(failure));
  process.exit(1);
}
