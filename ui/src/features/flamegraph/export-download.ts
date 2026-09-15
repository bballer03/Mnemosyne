export type ExportKind = "flamegraph" | "report";
export type FlamegraphExportFormat = "svg" | "folded-stack" | "json";
export type ReportExportFormat = "text" | "markdown" | "html" | "toon" | "json";

export type ExportDownload = {
  filename: string;
  mimeType: string;
  content: string;
  blob: Blob;
};

type PrepareExportInput =
  | {
      kind: "flamegraph";
      format: FlamegraphExportFormat;
      displayName: string;
      mode: string;
      content: unknown;
    }
  | {
      kind: "report";
      format: ReportExportFormat;
      displayName: string;
      mode: string;
      content: unknown;
    };

type DownloadDependencies = {
  createObjectURL: (blob: Blob) => string;
  revokeObjectURL: (url: string) => void;
  createAnchor: () => HTMLAnchorElement;
  appendChild: (anchor: HTMLAnchorElement) => void;
};

const MAX_FILENAME_LENGTH = 96;

const formatMetadata = {
  flamegraph: {
    svg: { extension: "svg", mimeType: "image/svg+xml" },
    "folded-stack": { extension: "folded", mimeType: "text/plain" },
    json: { extension: "json", mimeType: "application/json" },
  },
  report: {
    text: { extension: "txt", mimeType: "text/plain" },
    markdown: { extension: "md", mimeType: "text/markdown" },
    html: { extension: "html", mimeType: "text/html" },
    toon: { extension: "toon", mimeType: "application/x-toon" },
    json: { extension: "json", mimeType: "application/json" },
  },
} as const;

function stripUnsafeControls(value: string): string {
  return value.replace(/[\u0000-\u0008\u000B\u000C\u000E-\u001F\u007F]/g, "");
}

function normalizeContent(content: unknown): string {
  if (typeof content === "string") {
    return stripUnsafeControls(content);
  }
  return JSON.stringify(
    content,
    (_key, value: unknown) =>
      typeof value === "string" ? stripUnsafeControls(value) : value,
    2,
  );
}

function safeToken(value: string, fallback: string): string {
  const normalized = value
    .normalize("NFKD")
    .replace(/[\u0300-\u036f]/g, "")
    .replace(/[^A-Za-z0-9._-]+/g, "-")
    .replace(/^[.-]+|[.-]+$/g, "")
    .replace(/[-_.]{2,}/g, "-")
    .toLowerCase();
  return normalized || fallback;
}

function safeHeapBasename(displayName: string): string {
  const basename = displayName.split(/[\\/]/).pop() ?? "";
  const withoutHeapExtension = basename.replace(/\.(?:hprof|bin|json)$/i, "");
  return safeToken(withoutHeapExtension, "mnemosyne-heap");
}

export function prepareExportDownload(input: PrepareExportInput): ExportDownload {
  const metadata = (formatMetadata[input.kind] as Record<
    string,
    { extension: string; mimeType: string }
  >)[input.format];
  if (!metadata) {
    throw new Error(`Unsupported ${input.kind} export format: ${String(input.format)}`);
  }

  const suffix = `-${input.kind}-${safeToken(input.mode, "unknown")}.${metadata.extension}`;
  const maxBaseLength = Math.max(1, MAX_FILENAME_LENGTH - suffix.length);
  const base = safeHeapBasename(input.displayName).slice(0, maxBaseLength);
  const filename = `${base || "mnemosyne-heap"}${suffix}`;
  const content = normalizeContent(input.content);
  const blob = new Blob([content], { type: metadata.mimeType });

  return {
    filename,
    mimeType: metadata.mimeType,
    content,
    blob,
  };
}

export function downloadExport(
  prepared: ExportDownload,
  dependencies: DownloadDependencies = {
    createObjectURL: (blob) => URL.createObjectURL(blob),
    revokeObjectURL: (url) => URL.revokeObjectURL(url),
    createAnchor: () => document.createElement("a"),
    appendChild: (anchor) => document.body.appendChild(anchor),
  },
): void {
  const url = dependencies.createObjectURL(prepared.blob);
  const anchor = dependencies.createAnchor();
  anchor.href = url;
  anchor.download = prepared.filename;
  anchor.rel = "noopener";
  dependencies.appendChild(anchor);
  try {
    anchor.click();
  } finally {
    anchor.remove();
    dependencies.revokeObjectURL(url);
  }
}
