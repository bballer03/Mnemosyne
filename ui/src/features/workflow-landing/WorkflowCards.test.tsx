import "../../test/setup";

import { render, within } from "@testing-library/react";
import { describe, expect, it } from "bun:test";
import { MemoryRouter } from "react-router-dom";

import { WorkflowCards } from "./WorkflowCards";

describe("WorkflowCards", () => {
  it("renders tune_gc, traverse_object_graph, and compare_snapshots cards", () => {
    const view = render(
      <MemoryRouter>
        <WorkflowCards heapPath="fixture.hprof" />
      </MemoryRouter>,
    );
    const page = within(view.container);

    expect(page.getByLabelText(/tune gc workflow card/i)).toBeInTheDocument();
    expect(page.getByLabelText(/traverse object graph workflow card/i)).toBeInTheDocument();
    expect(page.getByLabelText(/compare snapshots workflow card/i)).toBeInTheDocument();
  });

  it("deep-links compare snapshots to the existing /compare route", () => {
    const view = render(
      <MemoryRouter>
        <WorkflowCards heapPath="fixture.hprof" />
      </MemoryRouter>,
    );
    const page = within(view.container);

    const link = page.getByRole("link", { name: /open comparison view/i });
    expect(link).toHaveAttribute("href", "/compare");
  });

  it("renders without crashing outside a router context", () => {
    const view = render(<WorkflowCards heapPath="fixture.hprof" />);
    const page = within(view.container);

    expect(page.getByText(/open comparison view/i)).toBeInTheDocument();
  });
});
