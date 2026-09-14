let remembered:
  | {
      sourceId: string;
      displayName: string;
    }
  | undefined;

export function rememberDesktopHeapSource(sourceId: string, displayName: string) {
  remembered = { sourceId, displayName };
}

export function getRememberedDesktopHeapSource():
  | { sourceId: string; displayName: string }
  | undefined {
  return remembered;
}

export function clearRememberedDesktopHeapSource() {
  remembered = undefined;
}
