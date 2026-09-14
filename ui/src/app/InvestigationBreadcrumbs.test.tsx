import "../test/setup";

import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it } from "bun:test";
import { MemoryRouter, Route, Routes } from "react-router-dom";

import { InvestigationBreadcrumbs } from "./InvestigationBreadcrumbs";

describe("InvestigationBreadcrumbs", () => {
  afterEach(() => {
    cleanup();
  });

  it("renders home plus the current explorer surface", () => {
    const view = render(
      <MemoryRouter initialEntries={["/heap-explorer/dominators"]}>
        <Routes>
          <Route path="*" element={<InvestigationBreadcrumbs />} />
        </Routes>
      </MemoryRouter>,
    );

    const nav = view.getByRole("navigation", { name: /investigation/i });
    expect(nav).toHaveTextContent("Home");
    expect(nav).toHaveTextContent("Dominators");
  });

  it("adds an object crumb when objectId is present outside the inspector", () => {
    const view = render(
      <MemoryRouter initialEntries={["/heap-explorer/dominators?objectId=0xabc"]}>
        <Routes>
          <Route path="*" element={<InvestigationBreadcrumbs />} />
        </Routes>
      </MemoryRouter>,
    );

    const link = view.getByRole("link", { name: /object 0xabc/i });
    expect(link.getAttribute("href")).toContain("objectId=0xabc");
  });
});
