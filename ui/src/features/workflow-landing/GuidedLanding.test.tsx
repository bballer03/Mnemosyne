import "../../test/setup";

import { render, within } from "@testing-library/react";
import { describe, expect, it } from "bun:test";
import { MemoryRouter } from "react-router-dom";

import { GuidedLanding } from "./GuidedLanding";

describe("GuidedLanding", () => {
  it("composes the triage summary, NL bar, workflow cards, and (omitted) recent heaps section", () => {
    const view = render(
      <MemoryRouter>
        <GuidedLanding heapPath="fixture.hprof" />
      </MemoryRouter>,
    );
    const page = within(view.container);

    expect(page.getByText(/ai triage summary/i)).toBeInTheDocument();
    expect(page.getByLabelText(/natural language input/i)).toBeInTheDocument();
    expect(page.getByLabelText(/tune gc workflow card/i)).toBeInTheDocument();
    expect(page.getByLabelText(/traverse object graph workflow card/i)).toBeInTheDocument();
    expect(page.getByLabelText(/compare snapshots workflow card/i)).toBeInTheDocument();
    // RecentHeapsList renders nothing without a listSnapshots bridge method.
    expect(page.queryByLabelText(/recent heaps/i)).not.toBeInTheDocument();
  });

  it("renders without crashing when no artifact heap path is present yet", () => {
    const view = render(
      <MemoryRouter>
        <GuidedLanding />
      </MemoryRouter>,
    );
    const page = within(view.container);

    expect(page.getByText(/ask a question or start a workflow/i)).toBeInTheDocument();
  });
});
