import "./install-dom";

import { afterEach, expect } from "bun:test";

import * as matchers from "@testing-library/jest-dom/matchers";
import { cleanup } from "@testing-library/react";

import { resetAllTestState } from "./reset-test-state";

expect.extend(matchers);

afterEach(() => {
  cleanup();
  resetAllTestState();
});
