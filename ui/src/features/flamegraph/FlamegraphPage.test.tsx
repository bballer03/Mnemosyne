import "../../test/setup";

import { cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it } from "bun:test";
import { MemoryRouter } from "react-router-dom";

import { clearRememberedDesktopHeapSource } from "../artifact-loader/desktop-heap-session";
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

    fireEvent.click(view.getByRole("button", { name: /generate svg/i }));

    await waitFor(() => {
      expect(view.getByRole("status")).toHaveTextContent(/desktop host bridge/i);
    });
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

    fireEvent.click(view.getByRole("button", { name: /generate svg/i }));

    await waitFor(() => {
      expect(view.getByRole("img", { name: /retained-size flamegraph/i })).toBeInTheDocument();
    });

    const image = view.getByRole("img", { name: /retained-size flamegraph/i });
    expect(image.getAttribute("src") ?? "").toMatch(/^blob:/);
    expect(view.container.querySelector("svg")).toBeNull();
    expect(view.container.innerHTML).not.toContain("<svg");
  });
});
