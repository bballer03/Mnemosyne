import "../../../test/setup";

import { render, within } from "@testing-library/react";
import { describe, expect, it } from "bun:test";
import { MemoryRouter } from "react-router-dom";

import { ModeRail } from "./ModeRail";

describe("ModeRail", () => {
  it("renders heap explorer mode links and selected target status", () => {
    const view = render(
      <MemoryRouter initialEntries={["/heap-explorer/object-inspector"]}>
        <ModeRail
          selectedObject={{
            objectId: "0xdeadbeef",
            className: "com.example.Cache",
            name: "com.example.Cache",
          }}
        />
      </MemoryRouter>,
    );
    const rail = within(view.container);

    expect(rail.getByRole("link", { name: /dominators/i })).toHaveAttribute(
      "href",
      "/heap-explorer/dominators",
    );
    expect(rail.getByRole("link", { name: /object inspector/i })).toHaveAttribute(
      "href",
      "/heap-explorer/object-inspector",
    );
    expect(rail.getByRole("link", { name: /query console/i })).toHaveAttribute(
      "href",
      "/heap-explorer/query-console",
    );
    expect(rail.getByText(/selected target/i)).toBeInTheDocument();
    expect(rail.getByText(/0xdeadbeef/i)).toBeInTheDocument();
    expect(rail.getByText(/recent targets appear here after selection\./i)).toBeInTheDocument();
  });
});
