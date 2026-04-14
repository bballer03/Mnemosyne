import "../test/setup";

import { render } from "@testing-library/react";
import { describe, expect, it } from "bun:test";

import { App } from "./App";

describe("App", () => {
  it("renders the Mnemosyne app shell heading", () => {
    const view = render(<App />);

    expect(view.getByRole("heading", { name: /mnemosyne/i })).toBeInTheDocument();
    expect(view.getByText(/load analysis artifact/i)).toBeInTheDocument();
  });

  it("renders the route content separately from the shell heading", () => {
    const view = render(<App />);

    expect(view.getAllByText(/load analysis artifact/i)).toHaveLength(1);
    expect(view.getByText(/choose an analysis artifact to begin/i)).toBeInTheDocument();
  });
});
