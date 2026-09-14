import "../test/setup";

import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it } from "bun:test";

import { App } from "./App";

describe("App", () => {
  afterEach(() => {
    cleanup();
  });

  it("renders the Mnemosyne app shell heading", () => {
    const view = render(<App />);

    expect(view.getByRole("heading", { name: /mnemosyne/i })).toBeInTheDocument();
    expect(view.getByRole("heading", { name: /load analysis artifact/i })).toBeInTheDocument();
  });

  it("renders the route content separately from the shell heading", () => {
    const view = render(<App />);

    expect(view.getByRole("heading", { name: /load analysis artifact/i })).toBeInTheDocument();
    expect(view.getByText(/choose an analysis artifact to begin/i)).toBeInTheDocument();
  });
});
