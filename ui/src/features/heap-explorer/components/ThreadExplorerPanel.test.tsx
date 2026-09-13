import "../../../test/setup";

import userEvent from "@testing-library/user-event";
import { cleanup, render, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "bun:test";

import type { AnalysisArtifact } from "../../../lib/analysis-types";

import { ThreadExplorerPanel } from "./ThreadExplorerPanel";

function buildArtifact(overrides?: Partial<AnalysisArtifact>): AnalysisArtifact {
  return {
    summary: {
      heapPath: "fixture.hprof",
      totalObjects: 20,
      totalSizeBytes: 987,
      generatedAt: "2026-04-14T00:00:00Z",
      totalRecords: 20,
    },
    leaks: [],
    recommendations: [],
    elapsedSeconds: 1,
    graph: { nodeCount: 5, edgeCount: 1, dominatorCount: 0, dominators: [] },
    provenance: [],
    ...overrides,
  };
}

// Real backend shape verified against `mnemosyne analyze --threads --format
// json` against a synthetic HPROF fixture with a two-frame stack trace and
// ROOT_JAVA_FRAME locals (Slice 14.C).
function buildThreadReport(): NonNullable<AnalysisArtifact["threadReport"]> {
  return {
    threads: [
      {
        objectId: 20480,
        name: "Thread-7",
        daemon: false,
        stackTrace: [
          {
            methodName: "run",
            className: "com/example/WorkerThread",
            sourceFile: "WorkerThread.java",
            lineNumber: 42,
            locals: [
              {
                variableSlot: 0,
                objectId: "0x00003000",
                className: "com.example.webapp.RequestHandler",
                rootKind: "JavaFrame",
              },
            ],
          },
          {
            methodName: "mainLoop",
            className: "com/example/WorkerThread",
            sourceFile: "WorkerThread.java",
            lineNumber: 17,
            locals: [],
          },
        ],
        retainedBytes: 2048,
        threadLocalCount: 3,
        threadLocalBytes: 512,
      },
    ],
    totalThreadCount: 1,
    totalThreadRetained: 2048,
    topRetainers: [],
  };
}

describe("ThreadExplorerPanel", () => {
  afterEach(() => {
    cleanup();
  });

  it("shows an explicit absent state when threadReport is missing from the artifact", () => {
    const view = render(<ThreadExplorerPanel artifact={buildArtifact()} />);

    expect(view.getByText(/thread analysis is absent from this artifact/i)).toBeInTheDocument();
  });

  it("shows an explicit empty state when the report is present but has no threads", () => {
    const artifact = buildArtifact({
      threadReport: { threads: [], totalThreadCount: 0, totalThreadRetained: 0, topRetainers: [] },
    });

    const view = render(<ThreadExplorerPanel artifact={artifact} />);

    expect(view.getByText(/this artifact contains no threads/i)).toBeInTheDocument();
  });

  it("selects the first thread by default and renders its frames with frame-locals", () => {
    const artifact = buildArtifact({ threadReport: buildThreadReport() });
    const view = render(<ThreadExplorerPanel artifact={artifact} />);
    const detail = within(view.getByRole("region", { name: /selected thread detail/i }));

    expect(detail.getByText("Thread-7")).toBeInTheDocument();
    expect(detail.getByText(/com\/example\/WorkerThread\.run/)).toBeInTheDocument();
    expect(detail.getByText(/WorkerThread\.java:42/)).toBeInTheDocument();
    expect(detail.getByText("0x00003000")).toBeInTheDocument();
    expect(detail.getByText("com.example.webapp.RequestHandler")).toBeInTheDocument();
    expect(detail.getByText("Java frame local")).toBeInTheDocument();
    expect(detail.getByText(/no locals rooted at this frame/i)).toBeInTheDocument();
  });

  it("switches the detail view when a different thread is selected", async () => {
    const user = userEvent.setup();
    const report = buildThreadReport();
    report.threads.push({
      objectId: 20481,
      name: "Thread-8",
      daemon: true,
      stackTrace: undefined,
      retainedBytes: 0,
      threadLocalCount: 0,
      threadLocalBytes: 0,
    });
    report.totalThreadCount = 2;

    const artifact = buildArtifact({ threadReport: report });
    const view = render(<ThreadExplorerPanel artifact={artifact} />);

    await user.click(view.getByRole("button", { name: /select thread-8/i }));

    const detail = within(view.getByRole("region", { name: /selected thread detail/i }));
    expect(detail.getByText("Thread-8")).toBeInTheDocument();
    expect(detail.getByText(/no stack trace was captured for this thread/i)).toBeInTheDocument();
  });
});
