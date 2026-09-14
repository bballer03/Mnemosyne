import "../../test/setup";

import { cleanup, render, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "bun:test";
import { MemoryRouter } from "react-router-dom";

import { clearRememberedDesktopHeapSource, rememberDesktopHeapSource } from "../artifact-loader/desktop-heap-session";
import { PolicyCheckPage } from "./PolicyCheckPage";

describe("PolicyCheckPage", () => {
  afterEach(() => {
    cleanup();
    clearRememberedDesktopHeapSource();
    delete window.__MNEMOSYNE_DESKTOP_HEAP_BRIDGE__;
  });

  it("shows an unavailable status when the desktop bridge is missing", async () => {
    const user = userEvent.setup();
    const view = render(
      <MemoryRouter>
        <PolicyCheckPage />
      </MemoryRouter>,
    );

    await user.click(view.getByRole("button", { name: /run policy check/i }));
    await waitFor(() => {
      expect(view.getByText(/requires the desktop host bridge/i)).toBeInTheDocument();
    });
  });

  it("renders violations and skipped rules without treating skip as pass", async () => {
    const user = userEvent.setup();
    rememberDesktopHeapSource("src-1", "fixture.hprof");
    window.__MNEMOSYNE_DESKTOP_HEAP_BRIDGE__ = {
      runCiCheck: async () => ({
        result: {
          mode_used: "OVERVIEW",
          mode_requested: "AUTO",
          violations: [],
          evaluations: [],
          skipped: [{ rule_id: "deep-only-rule", reason: "deep_only_in_overview" }],
        },
        exit_code: 0,
        fail_on: "error",
        evaluation_complete: false,
      }),
    };

    const view = render(
      <MemoryRouter>
        <PolicyCheckPage />
      </MemoryRouter>,
    );

    await user.click(view.getByRole("button", { name: /run policy check/i }));
    await waitFor(() => {
      expect(view.getByText(/Incomplete evaluation/i)).toBeInTheDocument();
    });
    expect(view.getByText(/Skipped \(not a green pass\)/i)).toBeInTheDocument();
    expect(view.getByText(/deep-only-rule/i)).toBeInTheDocument();
    expect(view.getByRole("note")).toHaveTextContent(/Incomplete evaluation/i);
    expect(view.getByText(/· incomplete/i)).toBeInTheDocument();
    expect(view.queryByText(/No violations at or above the evaluated rules/i)).toBeNull();
    expect(view.getByText(/skipped deep-only rules remain unproven/i)).toBeInTheDocument();
  });

  it("surfaces object_growth_threshold_requires_baseline as a structured error, not a skip", async () => {
    const user = userEvent.setup();
    rememberDesktopHeapSource("src-1", "fixture.hprof");
    window.__MNEMOSYNE_DESKTOP_HEAP_BRIDGE__ = {
      runCiCheck: async () => {
        throw new Error(
          "object_growth_threshold_requires_baseline: pass a baseline heap source for growth rules.",
        );
      },
    };

    const view = render(
      <MemoryRouter>
        <PolicyCheckPage />
      </MemoryRouter>,
    );

    await user.click(view.getByRole("button", { name: /run policy check/i }));

    await waitFor(() => {
      expect(view.getByRole("status")).toHaveTextContent(/object_growth_threshold_requires_baseline/i);
    });
    expect(view.queryByText(/Skipped \(not a green pass\)/i)).toBeNull();
    expect(view.queryByText(/Incomplete evaluation/i)).toBeNull();
  });

  it("surfaces a malformed policy error without rendering a fake green result", async () => {
    const user = userEvent.setup();
    rememberDesktopHeapSource("src-1", "fixture.hprof");
    window.__MNEMOSYNE_DESKTOP_HEAP_BRIDGE__ = {
      runCiCheck: async () => {
        throw new Error("invalid policy TOML: missing [[rule]] table");
      },
    };

    const view = render(
      <MemoryRouter>
        <PolicyCheckPage />
      </MemoryRouter>,
    );

    await user.click(view.getByRole("button", { name: /run policy check/i }));

    await waitFor(() => {
      expect(view.getByRole("status")).toHaveTextContent(/invalid policy TOML/i);
    });
    expect(view.queryByText(/No violations at or above the evaluated rules/i)).toBeNull();
    expect(view.queryByText(/exit_code=/i)).toBeNull();
  });
});
