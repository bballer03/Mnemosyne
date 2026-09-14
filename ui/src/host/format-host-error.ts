/**
 * Normalize host/Tauri invoke failures into a single human-readable string.
 *
 * Tauri often rejects with a plain string or `{ message }` object — not an
 * `Error` instance — so `error instanceof Error ? error.message : fallback`
 * silently drops the real reason.
 */
export function formatHostError(error: unknown, fallback: string): string {
  if (typeof error === "string") {
    const trimmed = error.trim();
    return trimmed.length > 0 ? trimmed : fallback;
  }

  if (error instanceof Error) {
    const trimmed = error.message.trim();
    return trimmed.length > 0 ? trimmed : fallback;
  }

  if (error && typeof error === "object") {
    const record = error as Record<string, unknown>;
    for (const key of ["message", "error", "msg"] as const) {
      const value = record[key];
      if (typeof value === "string" && value.trim().length > 0) {
        return value.trim();
      }
    }
  }

  return fallback;
}
