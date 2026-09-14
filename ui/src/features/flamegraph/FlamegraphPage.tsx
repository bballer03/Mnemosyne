import { useEffect, useState } from "react";
import { Link } from "react-router-dom";

import { pickHeapFile } from "../artifact-loader/desktop-heap-client";
import {
  getRememberedDesktopHeapSource,
  rememberDesktopHeapSource,
} from "../artifact-loader/desktop-heap-session";

type FlamegraphResult = {
  format: string;
  content: string;
  byteLength: number;
};

function getBridge():
  | {
      generateFlamegraph?: (input: {
        sourceId: string;
        root?: string;
        format?: string;
      }) => Promise<FlamegraphResult>;
    }
  | undefined {
  if (typeof window === "undefined") {
    return undefined;
  }
  return window.__MNEMOSYNE_DESKTOP_HEAP_BRIDGE__ as
    | {
        generateFlamegraph?: (input: {
          sourceId: string;
          root?: string;
          format?: string;
        }) => Promise<FlamegraphResult>;
      }
    | undefined;
}

export function FlamegraphPage() {
  const [root, setRoot] = useState("dominator");
  const [status, setStatus] = useState("Ready.");
  const [running, setRunning] = useState(false);
  const [svgUrl, setSvgUrl] = useState<string | undefined>();
  const [meta, setMeta] = useState<string | undefined>();

  useEffect(() => {
    return () => {
      if (svgUrl) {
        URL.revokeObjectURL(svgUrl);
      }
    };
  }, [svgUrl]);

  async function handleGenerate() {
    setRunning(true);
    setStatus("Generating…");
    try {
      const bridge = getBridge();
      if (!bridge?.generateFlamegraph) {
        setStatus("Flamegraphs require the desktop host bridge.");
        return;
      }

      let source = getRememberedDesktopHeapSource();
      if (!source) {
        const picked = await pickHeapFile();
        if (picked.status !== "selected") {
          setStatus(
            picked.status === "cancelled"
              ? "Heap selection cancelled."
              : "Open a heap dump in the desktop app first.",
          );
          return;
        }
        rememberDesktopHeapSource(picked.sourceId, picked.displayName);
        source = picked;
      }

      const result = await bridge.generateFlamegraph({
        sourceId: source.sourceId,
        root,
        format: "svg",
      });

      if (svgUrl) {
        URL.revokeObjectURL(svgUrl);
      }
      const blob = new Blob([result.content], { type: "image/svg+xml" });
      const url = URL.createObjectURL(blob);
      setSvgUrl(url);
      setMeta(`${result.byteLength.toLocaleString()} bytes · ${root} · ${source.displayName}`);
      setStatus("Flamegraph ready.");
    } catch (error) {
      setStatus(error instanceof Error ? error.message : "Flamegraph generation failed.");
    } finally {
      setRunning(false);
    }
  }

  return (
    <main style={{ display: "grid", gap: "1rem", padding: "1.5rem" }}>
      <div style={{ display: "flex", gap: "0.75rem", flexWrap: "wrap" }}>
        <Link to="/">Home</Link>
        <Link to="/workbench/flamegraphs" aria-current="page">
          Flamegraphs
        </Link>
      </div>
      <p
        style={{
          margin: 0,
          color: "#38bdf8",
          fontSize: "0.78rem",
          letterSpacing: "0.16em",
          textTransform: "uppercase",
        }}
      >
        Workbench
      </p>
      <h1 style={{ margin: 0, fontSize: "clamp(1.6rem, 3vw, 2.2rem)" }}>Flamegraphs</h1>
      <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.7, maxWidth: "68ch" }}>
        Retained-size flamegraph from the remembered desktop heap. SVG is rendered via object URL,
        never injected as HTML.
      </p>

      <div style={{ display: "flex", gap: "1rem", flexWrap: "wrap", alignItems: "center" }}>
        <label>
          root{" "}
          <select value={root} onChange={(event) => setRoot(event.target.value)}>
            <option value="dominator">dominator</option>
            <option value="class-hierarchy">class-hierarchy</option>
            <option value="gc-root-path">gc-root-path</option>
          </select>
        </label>
        <button type="button" onClick={() => void handleGenerate()} disabled={running}>
          {running ? "Generating…" : "Generate SVG"}
        </button>
      </div>

      <p role="status" style={{ margin: 0, color: "#cbd5e1" }}>
        {status}
      </p>
      {meta ? <p style={{ margin: 0, color: "#94a3b8" }}>{meta}</p> : null}
      {svgUrl ? (
        <img
          src={svgUrl}
          alt="Retained-size flamegraph"
          style={{ width: "100%", maxWidth: 1200, background: "#fff", borderRadius: 12 }}
        />
      ) : null}
    </main>
  );
}
