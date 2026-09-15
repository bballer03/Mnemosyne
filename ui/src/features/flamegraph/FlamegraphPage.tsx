import { useEffect, useState } from "react";
import { Link } from "react-router-dom";

import { pickHeapFile } from "../artifact-loader/desktop-heap-client";
import {
  getRememberedDesktopHeapSource,
  rememberDesktopHeapSource,
} from "../artifact-loader/desktop-heap-session";
import {
  downloadExport,
  prepareExportDownload,
  type ExportDownload,
  type FlamegraphExportFormat,
  type ReportExportFormat,
} from "./export-download";

type PreparedWorkspaceExport = {
  download: ExportDownload;
  format: string;
  byteLength: number;
  mode: string;
  provenance: Array<{ kind: string; detail?: string }>;
};

type DesktopHeapSource = {
  sourceId: string;
  displayName: string;
};

async function resolveSource(): Promise<DesktopHeapSource | undefined> {
  const remembered = getRememberedDesktopHeapSource();
  if (remembered) {
    return remembered;
  }
  const picked = await pickHeapFile();
  if (picked.status !== "selected") {
    return undefined;
  }
  rememberDesktopHeapSource(picked.sourceId, picked.displayName);
  return picked;
}

export function FlamegraphPage() {
  const [root, setRoot] = useState("dominator");
  const [flamegraphFormat, setFlamegraphFormat] =
    useState<FlamegraphExportFormat>("svg");
  const [reportFormat, setReportFormat] = useState<ReportExportFormat>("json");
  const [status, setStatus] = useState("Ready.");
  const [running, setRunning] = useState<"flamegraph" | "report">();
  const [previewUrl, setPreviewUrl] = useState<string>();
  const [flamegraphExport, setFlamegraphExport] =
    useState<PreparedWorkspaceExport>();
  const [reportExport, setReportExport] = useState<PreparedWorkspaceExport>();

  useEffect(
    () => () => {
      if (previewUrl) {
        URL.revokeObjectURL(previewUrl);
      }
    },
    [previewUrl],
  );

  async function handleGenerateFlamegraph() {
    const bridge = window.__MNEMOSYNE_DESKTOP_HEAP_BRIDGE__;
    if (!bridge?.generateFlamegraph) {
      setStatus("Flamegraphs require the desktop host bridge.");
      return;
    }
    setRunning("flamegraph");
    setStatus(`Generating ${flamegraphFormat} flamegraph…`);
    try {
      const source = await resolveSource();
      if (!source) {
        setStatus("Open a heap dump in the desktop app first.");
        return;
      }
      const selectedFormat = flamegraphFormat;
      const result = await bridge.generateFlamegraph({
        sourceId: source.sourceId,
        root,
        format: selectedFormat,
      });
      const mode = result.mode ?? "deep";
      const prepared = prepareExportDownload({
        kind: "flamegraph",
        format: selectedFormat,
        displayName: source.displayName,
        mode,
        content: result.content,
      });
      setPreviewUrl(
        selectedFormat === "svg" ? URL.createObjectURL(prepared.blob) : undefined,
      );
      setFlamegraphExport({
        download: prepared,
        format: selectedFormat,
        byteLength: result.byteLength,
        mode,
        provenance: result.provenance ?? [],
      });
      setStatus(`${selectedFormat} flamegraph ready.`);
    } catch (error) {
      setStatus(error instanceof Error ? error.message : "Flamegraph generation failed.");
    } finally {
      setRunning(undefined);
    }
  }

  async function handleGenerateReport() {
    const bridge = window.__MNEMOSYNE_DESKTOP_HEAP_BRIDGE__;
    if (!bridge?.exportReport) {
      setStatus("Report exports require the desktop host bridge and a current analysis.");
      return;
    }
    setRunning("report");
    setStatus(`Generating ${reportFormat} report…`);
    try {
      const source = await resolveSource();
      if (!source) {
        setStatus("Open and analyze a heap dump in the desktop app first.");
        return;
      }
      const selectedFormat = reportFormat;
      const result = await bridge.exportReport({
        sourceId: source.sourceId,
        format: selectedFormat,
      });
      const prepared = prepareExportDownload({
        kind: "report",
        format: selectedFormat,
        displayName: source.displayName,
        mode: result.mode,
        content: result.content,
      });
      setReportExport({
        download: prepared,
        format: selectedFormat,
        byteLength: result.byteLength,
        mode: result.mode,
        provenance: result.provenance,
      });
      setStatus(`${selectedFormat} report ready.`);
    } catch (error) {
      setStatus(error instanceof Error ? error.message : "Report export failed.");
    } finally {
      setRunning(undefined);
    }
  }

  const renderMetadata = (exported: PreparedWorkspaceExport) => (
    <div style={{ display: "grid", gap: "0.35rem", color: "#cbd5e1" }}>
      <span>
        Format: {exported.format} · {exported.byteLength.toLocaleString()} bytes ·
        Mode: {exported.mode}
      </span>
      <span>File: {exported.download.filename}</span>
      {exported.provenance.length > 0 ? (
        <ul aria-label="Provenance" style={{ margin: 0, paddingLeft: "1.25rem" }}>
          {exported.provenance.map((marker, index) => (
            <li key={`${marker.kind}-${marker.detail ?? ""}-${index}`}>
              {marker.kind}
              {marker.detail ? ` · ${marker.detail}` : ""}
            </li>
          ))}
        </ul>
      ) : null}
    </div>
  );

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
      <h1 style={{ margin: 0, fontSize: "clamp(1.6rem, 3vw, 2.2rem)" }}>
        Flamegraphs and exports
      </h1>
      <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.7, maxWidth: "68ch" }}>
        Export flamegraphs and native analysis reports from the remembered desktop
        heap. SVG previews use object URLs; report and folded/JSON bodies are never
        mounted in the page.
      </p>

      <section style={{ display: "grid", gap: "0.75rem" }}>
        <h2 style={{ margin: 0, fontSize: "1.15rem" }}>Flamegraph</h2>
        <div
          style={{
            display: "flex",
            gap: "1rem",
            flexWrap: "wrap",
            alignItems: "center",
          }}
        >
          <label>
            Root{" "}
            <select value={root} onChange={(event) => setRoot(event.target.value)}>
              <option value="dominator">dominator</option>
              <option value="class-hierarchy">class-hierarchy</option>
              <option value="gc-root-path">gc-root-path</option>
            </select>
          </label>
          <label>
            Flamegraph format{" "}
            <select
              value={flamegraphFormat}
              onChange={(event) =>
                setFlamegraphFormat(event.target.value as FlamegraphExportFormat)
              }
            >
              <option value="svg">SVG</option>
              <option value="folded-stack">Folded stack</option>
              <option value="json">JSON</option>
            </select>
          </label>
          <button
            type="button"
            onClick={() => void handleGenerateFlamegraph()}
            disabled={running !== undefined}
          >
            {running === "flamegraph" ? "Generating…" : "Generate flamegraph"}
          </button>
          {flamegraphExport ? (
            <button
              type="button"
              onClick={() => downloadExport(flamegraphExport.download)}
            >
              Download flamegraph
            </button>
          ) : null}
        </div>
        {flamegraphExport ? renderMetadata(flamegraphExport) : null}
      </section>

      {previewUrl ? (
        <img
          src={previewUrl}
          alt="Retained-size flamegraph"
          style={{ width: "100%", maxWidth: 1200, background: "#fff", borderRadius: 12 }}
        />
      ) : null}

      <section style={{ display: "grid", gap: "0.75rem" }}>
        <h2 style={{ margin: 0, fontSize: "1.15rem" }}>Analysis report</h2>
        <div
          style={{
            display: "flex",
            gap: "1rem",
            flexWrap: "wrap",
            alignItems: "center",
          }}
        >
          <label>
            Report format{" "}
            <select
              value={reportFormat}
              onChange={(event) =>
                setReportFormat(event.target.value as ReportExportFormat)
              }
            >
              <option value="text">Text</option>
              <option value="markdown">Markdown</option>
              <option value="html">HTML</option>
              <option value="toon">TOON</option>
              <option value="json">JSON</option>
            </select>
          </label>
          <button
            type="button"
            onClick={() => void handleGenerateReport()}
            disabled={running !== undefined}
          >
            {running === "report" ? "Generating…" : "Generate report"}
          </button>
          {reportExport ? (
            <button type="button" onClick={() => downloadExport(reportExport.download)}>
              Download report
            </button>
          ) : null}
        </div>
        {reportExport ? renderMetadata(reportExport) : null}
      </section>

      <p role="status" style={{ margin: 0, color: "#cbd5e1" }}>
        {status}
      </p>
    </main>
  );
}
