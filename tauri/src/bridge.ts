/**
 * DEPRECATED — do not load from here.
 *
 * Host bridge injection lives in the Vite UI bundle so it ships with
 * `frontendDist` (`../ui/dist`):
 *   `ui/src/host/tauri-bridge.ts` → bootstrapped from `ui/src/main.tsx`
 *
 * This file previously defined the same `window.__MNEMOSYNE_*_BRIDGE__`
 * wiring but was never imported into the UI build, so packaged desktop
 * builds looked native while behaving like a browser (Open heap dump
 * showed "available in the desktop app").
 *
 * Keep this stub only as a pointer for older docs/search hits.
 */
export {};
