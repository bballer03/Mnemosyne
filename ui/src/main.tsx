import React from "react";
import ReactDOM from "react-dom/client";

import { App } from "@/app/App";
import { injectHostBridges } from "@/host/tauri-bridge";
import "@/app/globals.css";

async function bootstrap() {
  // Desktop: inject window.__MNEMOSYNE_*_BRIDGE__ before React mounts so Home
  // "Open heap dump" sees pickHeapFile on first click. Browser: no-op.
  try {
    await injectHostBridges();
  } catch (error) {
    console.error("[mnemosyne] host bridge injection failed", error);
  }

  ReactDOM.createRoot(document.getElementById("root")!).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
}

void bootstrap();
