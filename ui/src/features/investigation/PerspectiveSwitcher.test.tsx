import "../../test/setup";

import { beforeEach, describe, expect, it } from "bun:test";
import { fireEvent, render } from "@testing-library/react";
import { MemoryRouter, useLocation } from "react-router-dom";

import { useInvestigationStore } from "./investigation-store";
import { PerspectiveSwitcher } from "./PerspectiveSwitcher";

function LocationProbe() {
  return <output aria-label="Current route">{useLocation().pathname}</output>;
}

describe("PerspectiveSwitcher", () => {
  beforeEach(() => {
    window.sessionStorage.clear();
    useInvestigationStore.setState({
      perspectiveId: "leak-hunt",
      persistenceIdentity: undefined,
    });
  });

  it("applies a named perspective and switches to another fixed layout", () => {
    const view = render(
      <MemoryRouter initialEntries={["/dashboard"]}>
        <PerspectiveSwitcher />
        <LocationProbe />
      </MemoryRouter>,
    );

    expect(view.getByRole("heading", { name: "Workbench perspective" })).toBeInTheDocument();
    expect(view.getByRole("button", { name: /Leak Hunt/ })).toHaveAttribute(
      "aria-pressed",
      "true",
    );

    fireEvent.click(view.getByRole("button", { name: /Dominator Browse/ }));
    expect(useInvestigationStore.getState().perspectiveId).toBe("dominator-browse");
    expect(view.getByLabelText("Current route")).toHaveTextContent(
      "/heap-explorer/dominators",
    );
    expect(view.getByRole("button", { name: /Dominator Browse/ })).toHaveAttribute(
      "aria-pressed",
      "true",
    );

    fireEvent.click(view.getByRole("button", { name: /OQL Lab/ }));
    expect(useInvestigationStore.getState().perspectiveId).toBe("oql-lab");
    expect(view.getByLabelText("Current route")).toHaveTextContent(
      "/heap-explorer/query-console",
    );
  });

  it("persists the selected preset with workspace metadata", () => {
    const identity = { kind: "workspace" as const, key: "fixture-source" };
    useInvestigationStore.getState().activatePersistence(identity, {
      identity,
      revision: 0,
      objectIds: new Set<string>(),
      classKeys: new Set<string>(),
      leakIds: new Set<string>(),
    });

    useInvestigationStore.getState().setPerspectiveId("compare");
    useInvestigationStore.setState({
      perspectiveId: "leak-hunt",
      persistenceIdentity: undefined,
    });
    useInvestigationStore.getState().activatePersistence(identity, {
      identity,
      revision: 0,
      objectIds: new Set<string>(),
      classKeys: new Set<string>(),
      leakIds: new Set<string>(),
    });

    expect(useInvestigationStore.getState().perspectiveId).toBe("compare");
  });
});
