import "../test/setup";

import { describe, expect, it } from "bun:test";

import { formatHostError } from "./format-host-error";

describe("formatHostError", () => {
  it("prefers Error.message", () => {
    expect(formatHostError(new Error("Unknown heap source"), "fallback")).toBe(
      "Unknown heap source",
    );
  });

  it("accepts plain string rejects from Tauri", () => {
    expect(formatHostError("Selected file must use a .hprof or .bin extension", "fallback")).toBe(
      "Selected file must use a .hprof or .bin extension",
    );
  });

  it("reads message from plain objects", () => {
    expect(formatHostError({ message: "Heap session lock poisoned" }, "fallback")).toBe(
      "Heap session lock poisoned",
    );
  });

  it("uses fallback when nothing useful is present", () => {
    expect(formatHostError(null, "Failed to open heap dump")).toBe("Failed to open heap dump");
    expect(formatHostError({}, "Failed to open heap dump")).toBe("Failed to open heap dump");
  });
});
