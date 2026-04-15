import "../../../test/setup";

import { cleanup, render, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "bun:test";
import { MemoryRouter } from "react-router-dom";

import { ExplorerCrossNavActions } from "./ExplorerCrossNavActions";

describe("ExplorerCrossNavActions", () => {
  afterEach(() => {
    cleanup();
  });

  it("links from a selected object to object inspector and query console routes", () => {
    const view = render(
      <MemoryRouter>
        <ExplorerCrossNavActions leakId="leak-1" objectId="cache root/0x2a?" />
      </MemoryRouter>,
    );
    const panel = within(view.container);

    expect(panel.getByRole("link", { name: /open object inspector/i })).toHaveAttribute(
      "href",
      "/heap-explorer/object-inspector?objectId=cache%20root%2F0x2a%3F",
    );
    expect(panel.getByRole("link", { name: /open query console/i })).toHaveAttribute(
      "href",
      "/heap-explorer/query-console?objectId=cache%20root%2F0x2a%3F",
    );
  });

  it("falls back to base explorer routes when no object id is provided", () => {
    const view = render(
      <MemoryRouter>
        <ExplorerCrossNavActions />
      </MemoryRouter>,
    );
    const panel = within(view.container);

    expect(panel.getByRole("link", { name: /open object inspector/i })).toHaveAttribute(
      "href",
      "/heap-explorer/object-inspector",
    );
    expect(panel.getByRole("link", { name: /open query console/i })).toHaveAttribute(
      "href",
      "/heap-explorer/query-console",
    );
  });

  it("only renders the leak workspace link when a leak id is provided", () => {
    const { rerender } = render(
      <MemoryRouter>
        <ExplorerCrossNavActions objectId="0x2a" />
      </MemoryRouter>,
    );
    const initialPanel = within(document.body);

    expect(initialPanel.queryByRole("link", { name: /open leak workspace/i })).toBeNull();

    rerender(
      <MemoryRouter>
        <ExplorerCrossNavActions leakId="leak with spaces/1" objectId="0x2a" />
      </MemoryRouter>,
    );
    const rerenderedPanel = within(document.body);

    expect(rerenderedPanel.getByRole("link", { name: /open leak workspace/i })).toHaveAttribute(
      "href",
      "/leaks/leak%20with%20spaces%2F1/overview",
    );
  });
});
