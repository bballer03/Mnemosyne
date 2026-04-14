import { useRef, useState } from "react";

type ArtifactDropzoneProps = {
  disabled?: boolean;
  onFileSelected: (file: File) => void | Promise<void>;
};

export function ArtifactDropzone({ disabled = false, onFileSelected }: ArtifactDropzoneProps) {
  const inputRef = useRef<HTMLInputElement | null>(null);
  const [isActive, setIsActive] = useState(false);

  function handleFile(file: File | undefined) {
    if (!file || disabled) {
      return;
    }

    void onFileSelected(file);
  }

  return (
    <section
      aria-label="Local artifact dropzone"
      onDragEnter={(event) => {
        event.preventDefault();
        if (!disabled) {
          setIsActive(true);
        }
      }}
      onDragOver={(event) => {
        event.preventDefault();
      }}
      onDragLeave={(event) => {
        event.preventDefault();
        setIsActive(false);
      }}
      onDrop={(event) => {
        event.preventDefault();
        setIsActive(false);
        handleFile(event.dataTransfer.files?.[0]);
      }}
      style={{
        border: `1px dashed ${isActive ? "#7dd3fc" : "#334155"}`,
        borderRadius: 20,
        background: isActive ? "rgba(14, 165, 233, 0.12)" : "rgba(15, 23, 42, 0.92)",
        padding: "1.5rem",
      }}
    >
      <div
        style={{
          display: "flex",
          flexDirection: "column",
          gap: "0.75rem",
        }}
      >
        <div>
          <p
            style={{
              margin: 0,
              fontSize: "0.78rem",
              letterSpacing: "0.14em",
              textTransform: "uppercase",
              color: "#38bdf8",
            }}
          >
            Local Artifact Input
          </p>
          <label
            htmlFor="artifact-loader-input"
            style={{
              display: "block",
              marginTop: "0.5rem",
              fontSize: "1.05rem",
              fontWeight: 600,
              color: "#e2e8f0",
            }}
          >
            Analysis JSON artifact
          </label>
        </div>

        <p
          style={{
            margin: 0,
            color: "#94a3b8",
            lineHeight: 1.6,
          }}
        >
          Drop a local `.json` file here or browse your filesystem. Expects Mnemosyne analysis
          JSON derived from AnalyzeResponse.
        </p>

        <div
          style={{
            display: "flex",
            flexWrap: "wrap",
            gap: "0.75rem",
            alignItems: "center",
          }}
        >
          <button
            type="button"
            disabled={disabled}
            onClick={() => inputRef.current?.click()}
            style={{
              border: "1px solid #38bdf8",
              borderRadius: 999,
              background: disabled ? "#0f172a" : "#082f49",
              color: "#e0f2fe",
              padding: "0.7rem 1rem",
              cursor: disabled ? "not-allowed" : "pointer",
            }}
          >
            Select local JSON
          </button>
          <span style={{ color: "#64748b", fontSize: "0.92rem" }}>
            Browser-only import. No upload or server sync.
          </span>
        </div>

        <input
          ref={inputRef}
          id="artifact-loader-input"
          aria-label="Analysis JSON artifact"
          type="file"
          accept="application/json,.json"
          disabled={disabled}
          onChange={(event) => {
            handleFile(event.currentTarget.files?.[0]);
            event.currentTarget.value = "";
          }}
          style={{
            position: "absolute",
            width: 1,
            height: 1,
            padding: 0,
            margin: -1,
            overflow: "hidden",
            clip: "rect(0, 0, 0, 0)",
            whiteSpace: "nowrap",
            border: 0,
          }}
        />
      </div>
    </section>
  );
}
