import {
  getRememberedDesktopHeapSource,
  rememberDesktopHeapSource,
} from "../artifact-loader/desktop-heap-session";
import { getDesktopHeapBridge, pickHeapFile } from "../artifact-loader/desktop-heap-client";

export type CiCheckInput = {
  sourceId: string;
  policyToml: string;
  failOn?: "info" | "warning" | "error" | "critical";
  mode?: "auto" | "deep" | "overview";
  baselineSourceId?: string;
};

export type CiCheckResponse = {
  result: {
    mode_used: string;
    mode_requested: string;
    violations: Array<{
      rule_id: string;
      predicate: string;
      severity: string;
      message: string;
      remediation_hint?: string | null;
    }>;
    evaluations: Array<{
      rule_id: string;
      passed: boolean;
    }>;
    skipped: Array<{
      rule_id: string;
      reason: string | { [key: string]: unknown };
    }>;
  };
  exit_code: number;
  fail_on: string;
  evaluation_complete: boolean;
};

function getBridge() {
  return getDesktopHeapBridge();
}

export function isCiCheckAvailable(): boolean {
  return typeof getBridge()?.runCiCheck === "function";
}

export async function runCiCheck(input: CiCheckInput): Promise<
  | { status: "unavailable" }
  | { status: "ready"; data: CiCheckResponse }
  | { status: "error"; error: string }
> {
  const bridge = getBridge();
  if (!bridge?.runCiCheck) {
    return { status: "unavailable" };
  }

  try {
    const raw = await bridge.runCiCheck(input);
    const data = raw as CiCheckResponse;
    if (typeof data.evaluation_complete !== "boolean") {
      data.evaluation_complete = (data.result?.skipped?.length ?? 0) === 0;
    }
    return { status: "ready", data };
  } catch (error) {
    return {
      status: "error",
      error: error instanceof Error ? error.message : "Unknown ci_check failure.",
    };
  }
}

export async function ensureDesktopHeapSource(): Promise<
  | { status: "selected"; sourceId: string; displayName: string }
  | { status: "cancelled" }
  | { status: "unavailable" }
  | { status: "error"; error: string }
> {
  const remembered = getRememberedDesktopHeapSource();
  if (remembered) {
    return { status: "selected", ...remembered };
  }

  try {
    const picked = await pickHeapFile();
    if (picked.status !== "selected") {
      return picked;
    }
    rememberDesktopHeapSource(picked.sourceId, picked.displayName);
    return picked;
  } catch (error) {
    return {
      status: "error",
      error: error instanceof Error ? error.message : "Failed to pick heap dump.",
    };
  }
}
