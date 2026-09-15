/** Shared workbench inline styles that reference `--mn-*` CSS custom properties. */

export const COMPACT_LAYOUT_MAX_WIDTH = 980;

export const workbenchPanelStyle = {
  border: "1px solid var(--mn-border-subtle)",
  borderRadius: "var(--mn-radius-panel)",
  background: "var(--mn-surface-raised)",
  padding: "var(--mn-space-panel)",
} as const;

export const workbenchEyebrowStyle = {
  margin: 0,
  color: "var(--mn-text-accent)",
  fontSize: "0.78rem",
  letterSpacing: "0.16em",
  textTransform: "uppercase" as const,
};

export const workbenchMutedStyle = {
  margin: 0,
  color: "var(--mn-text-muted)",
  lineHeight: 1.7,
  maxWidth: "68ch",
} as const;

export const workbenchAccentBorderStyle = {
  border: "1px solid var(--mn-border-accent)",
} as const;

/** One-column grid when compact; otherwise the caller-supplied template. */
export function compactGridColumns(
  isCompact: boolean,
  wideColumns: string,
): string {
  return isCompact ? "minmax(0, 1fr)" : wideColumns;
}
