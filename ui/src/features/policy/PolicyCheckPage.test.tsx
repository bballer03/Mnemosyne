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
          mode_used: "DEEP",
          mode_requested: "DEEP",
          violations: [
            {
              rule_id: "leak-budget",
              predicate: "leak_count",
              severity: "error",
              message: "too many leaks",
            },
          ],
          evaluations: [{ rule_id: "leak-budget", passed: false }],
          skipped: [{ rule_id: "deep-only-rule", reason: "deep_only_in_overview" }],
        },
        exit_code: 1,
        fail_on: "error",
      }),
    };

    const view = render(
      <MemoryRouter>
        <PolicyCheckPage />
      </MemoryRouter>,
    );

    await user.click(view.getByRole("button", { name: /run policy check/i }));
    await waitFor(() => {
      expect(view.getByText(/too many leaks/i)).toBeInTheDocument();
    });
    expect(view.getByText(/Skipped \(not a green pass\)/i)).toBeInTheDocument();
    expect(view.getByText(/deep-only-rule/i)).toBeInTheDocument();
  });
});
