import "../../test/setup";

import { cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it } from "bun:test";
import { MemoryRouter } from "react-router-dom";

import {
  clearRememberedDesktopHeapSource,
  rememberDesktopHeapSource,
} from "../artifact-loader/desktop-heap-session";
import { FlamegraphPage } from "./FlamegraphPage";

describe("FlamegraphPage", () => {
  afterEach(() => {
    cleanup();
    delete window.__MNEMOSYNE_DESKTOP_HEAP_BRIDGE__;
    clearRememberedDesktopHeapSource();
  });

  it("shows an unavailable status when the desktop bridge is missing", async () => {
    const view = render(
      <MemoryRouter>
        <FlamegraphPage />
      </MemoryRouter>,
    );

    fireEvent.click(view.getByRole("button", { name: /generate flamegraph/i }));

    await waitFor(() => {
      expect(view.getByRole("status")).toHaveTextContent(/desktop host bridge/i);
    });

    fireEvent.click(view.getByRole("button", { name: /generate report/i }));
    await waitFor(() => {
      expect(view.getByRole("status")).toHaveTextContent(/report exports require the desktop host bridge/i);
    });
  });

  it("surfaces overview-mode unavailability from the host without rendering SVG", async () => {
    rememberDesktopHeapSource("src-1", "fixture.hprof");
    window.__MNEMOSYNE_DESKTOP_HEAP_BRIDGE__ = {
      generateFlamegraph: async () => {
        throw new Error(
          "feature_unavailable_in_overview_mode: generate_flamegraph requires deep mode",
        );
      },
    };

    const view = render(
      <MemoryRouter>
        <FlamegraphPage />
      </MemoryRouter>,
    );

    fireEvent.click(view.getByRole("button", { name: /generate flamegraph/i }));

    await waitFor(() => {
      expect(view.getByRole("status")).toHaveTextContent(/feature_unavailable_in_overview_mode/i);
    });
    expect(view.queryByRole("img", { name: /retained-size flamegraph/i })).toBeNull();
    expect(view.container.querySelector("svg")).toBeNull();
  });

  it("renders SVG via an object URL image, never as injected HTML", async () => {
    const svg = '<svg xmlns="http://www.w3.org/2000/svg"><title>demo</title></svg>';
    window.__MNEMOSYNE_DESKTOP_HEAP_BRIDGE__ = {
      pickHeapFile: async () => ({
        status: "selected",
        sourceId: "src-1",
        displayName: "fixture.hprof",
      }),
      loadHeapFromSource: async () => ({
        displayName: "fixture.hprof",
        objectCount: 1,
        classCount: 1,
        gcRootCount: 1,
      }),
      runDesktopAnalysis: async () => ({}),
      generateFlamegraph: async () => ({
        format: "svg",
        content: svg,
        byteLength: svg.length,
      }),
    };

    const view = render(
      <MemoryRouter>
        <FlamegraphPage />
      </MemoryRouter>,
    );

    fireEvent.click(view.getByRole("button", { name: /generate flamegraph/i }));

    await waitFor(() => {
      expect(view.getByRole("img", { name: /retained-size flamegraph/i })).toBeInTheDocument();
    });

    const image = view.getByRole("img", { name: /retained-size flamegraph/i });
    expect(image.getAttribute("src") ?? "").toMatch(/^blob:/);
    expect(view.container.querySelector("svg")).toBeNull();
    expect(view.container.innerHTML).not.toContain("<svg");
  });

  it("exposes folded-stack and JSON flamegraph downloads without mounting their content", async () => {
    rememberDesktopHeapSource("src-1", "fixture.hprof");
    const calls: unknown[] = [];
    window.__MNEMOSYNE_DESKTOP_HEAP_BRIDGE__ = {
      generateFlamegraph: async (input) => {
        calls.push(input);
        return {
          format: input.format ?? "svg",
          content:
            input.format === "json"
              ? { title: "<script>FLAME_SENTINEL</script>", stacks: [] }
              : "root;child 42",
          byteLength: 13,
        };
      },
    };
    const view = render(
      <MemoryRouter>
        <FlamegraphPage />
      </MemoryRouter>,
    );

    fireEvent.change(view.getByLabelText(/flamegraph format/i), {
      target: { value: "folded-stack" },
    });
    fireEvent.click(view.getByRole("button", { name: /generate flamegraph/i }));
    await waitFor(() => expect(calls).toContainEqual({
      sourceId: "src-1",
      root: "dominator",
      format: "folded-stack",
    }));
    expect(view.queryByRole("img", { name: /retained-size flamegraph/i })).toBeNull();
    expect(view.getByRole("button", { name: /download flamegraph/i })).toBeInTheDocument();

    fireEvent.change(view.getByLabelText(/flamegraph format/i), {
      target: { value: "json" },
    });
    fireEvent.click(view.getByRole("button", { name: /generate flamegraph/i }));
    await waitFor(() => expect(calls).toContainEqual({
      sourceId: "src-1",
      root: "dominator",
      format: "json",
    }));
    expect(view.container.innerHTML).not.toContain("FLAME_SENTINEL");
    expect(view.container.querySelector("script")).toBeNull();
  });

  it("downloads native report formats with mode and provenance labels but no HTML injection", async () => {
    rememberDesktopHeapSource(
      "src-1",
      "../../<img src=x onerror=REPORT_SENTINEL>.hprof",
    );
    const calls: unknown[] = [];
    window.__MNEMOSYNE_DESKTOP_HEAP_BRIDGE__ = {
      exportReport: async (input) => {
        calls.push(input);
        return {
          format: input.format,
          content:
            input.format === "json"
              ? { sentinel: "<script>REPORT_SENTINEL</script>" }
              : "<html><script>REPORT_SENTINEL</script><img src=x onerror=REPORT_SENTINEL></html>",
          mimeType: input.format === "json" ? "application/json" : "text/html",
          byteLength: 91,
          mode: "deep",
          provenance: [
            { kind: "Partial", detail: "bounded analyzer output" },
            { kind: "Fallback", detail: "heuristic leak ranking" },
          ],
        };
      },
    };
    const view = render(
      <MemoryRouter>
        <FlamegraphPage />
      </MemoryRouter>,
    );

    fireEvent.change(view.getByLabelText(/report format/i), {
      target: { value: "html" },
    });
    fireEvent.click(view.getByRole("button", { name: /generate report/i }));

    await waitFor(() => expect(calls).toEqual([{ sourceId: "src-1", format: "html" }]));
    expect(view.getByText(/mode:\s*deep/i)).toBeInTheDocument();
    expect(view.getByText(/Partial · bounded analyzer output/i)).toBeInTheDocument();
    expect(view.getByText(/Fallback · heuristic leak ranking/i)).toBeInTheDocument();
    expect(
      view.getByText(
        "File: img-src-x-onerror-report_sentinel-report-deep.html",
      ),
    ).toBeInTheDocument();
    expect(view.getByRole("button", { name: /download report/i })).toBeInTheDocument();
    expect(view.container.innerHTML).not.toContain("REPORT_SENTINEL");
    expect(view.container.querySelector("script")).toBeNull();
    expect(view.container.querySelector("[onerror]")).toBeNull();

    fireEvent.change(view.getByLabelText(/report format/i), {
      target: { value: "json" },
    });
    fireEvent.click(view.getByRole("button", { name: /generate report/i }));
    await waitFor(() =>
      expect(calls).toContainEqual({ sourceId: "src-1", format: "json" }),
    );
    expect(view.container.innerHTML).not.toContain("REPORT_SENTINEL");
  });
});
