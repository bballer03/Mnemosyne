import { useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";

import { isTauriRuntime } from "../../host/tauri-bridge";
import { applyOpenedHeap, openDesktopHeapLean } from "./workspace-actions";

const OPEN_HEAP_MENU_EVENT = "mnemosyne://open-heap-requested";

export type DesktopOpenHeapMenuHost = {
  listen: (listener: () => void) => Promise<() => void>;
};

const tauriMenuHost: DesktopOpenHeapMenuHost = {
  listen: async (listener) => {
    const { listen } = await import("@tauri-apps/api/event");
    return listen(OPEN_HEAP_MENU_EVENT, listener);
  },
};

function getDesktopOpenHeapMenuHost(): DesktopOpenHeapMenuHost | undefined {
  return isTauriRuntime() ? tauriMenuHost : undefined;
}

/**
 * Routes the native File -> Open Heap menu through the same opaque-source
 * picker and analysis path as the existing desktop buttons.
 */
export function DesktopOpenHeapMenuHandler({
  menuHost = getDesktopOpenHeapMenuHost(),
}: {
  menuHost?: DesktopOpenHeapMenuHost;
} = {}) {
  const navigate = useNavigate();
  const inFlight = useRef(false);
  const [message, setMessage] = useState<string>();

  useEffect(() => {
    if (!menuHost) {
      return;
    }

    let disposed = false;
    let unlisten: (() => void) | undefined;
    const handleOpen = async () => {
      if (inFlight.current) {
        return;
      }
      inFlight.current = true;
      setMessage("Opening heap dump...");
      try {
        const result = await openDesktopHeapLean();
        if (disposed) {
          return;
        }
        if (result.status === "cancelled") {
          setMessage("Heap dump selection cancelled.");
          return;
        }
        if (result.status === "unavailable" || result.status === "error") {
          setMessage(result.message);
          return;
        }

        applyOpenedHeap(result.displayName, result.artifact, result.sourceId);
        setMessage(`Opened ${result.displayName}.`);
        navigate("/dashboard");
      } finally {
        inFlight.current = false;
      }
    };

    void menuHost.listen(() => void handleOpen()).then((cleanup) => {
      if (disposed) {
        cleanup();
      } else {
        unlisten = cleanup;
      }
    });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [menuHost, navigate]);

  return message ? (
    <p role="status" style={{ margin: 0, color: "#cbd5e1" }}>
      {message}
    </p>
  ) : null;
}
