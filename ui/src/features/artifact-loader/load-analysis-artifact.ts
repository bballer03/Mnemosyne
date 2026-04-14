import { parseAnalysisArtifact } from "../../lib/analysis-types";

export function loadAnalysisArtifactFromText(text: string) {
  let parsed: unknown;

  try {
    parsed = JSON.parse(text);
  } catch {
    throw new Error("Invalid JSON artifact: could not parse analysis JSON");
  }

  return parseAnalysisArtifact(parsed);
}
