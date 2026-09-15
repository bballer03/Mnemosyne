import { useEffect, useState } from "react";

import { COMPACT_LAYOUT_MAX_WIDTH } from "./theme-tokens";

/** True when the viewport is narrower than the documented workbench breakpoint. */
export function useCompactLayout(): boolean {
  const [isCompact, setIsCompact] = useState(
    () => (typeof window !== "undefined" ? window.innerWidth < COMPACT_LAYOUT_MAX_WIDTH : false),
  );

  useEffect(() => {
    function onResize() {
      setIsCompact(window.innerWidth < COMPACT_LAYOUT_MAX_WIDTH);
    }
    window.addEventListener("resize", onResize);
    onResize();
    return () => window.removeEventListener("resize", onResize);
  }, []);

  return isCompact;
}
