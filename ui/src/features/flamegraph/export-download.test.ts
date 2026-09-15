import "../../test/setup";

import { describe, expect, it, mock } from "bun:test";

import {
  downloadExport,
  prepareExportDownload,
} from "./export-download";

describe("export-download", () => {
  it("creates a bounded basename-only filename from untrusted display text", () => {
    const prepared = prepareExportDownload({
      kind: "report",
      format: "html",
      displayName: "../../<img src=x onerror=alert(1)>.hprof",
      mode: "deep",
      content: "<html>safe renderer output</html>",
    });

    expect(prepared.filename).toBe("img-src-x-onerror-alert-1-report-deep.html");
    expect(prepared.filename).not.toMatch(/[\\/<>"']/);
    expect(prepared.filename).not.toContain("..");
    expect(prepared.mimeType).toBe("text/html");

    const fallback = prepareExportDownload({
      kind: "flamegraph",
      format: "svg",
      displayName: "////",
      mode: "deep",
      content: "<svg />",
    });
    expect(fallback.filename).toBe("mnemosyne-heap-flamegraph-deep.svg");

    const bounded = prepareExportDownload({
      kind: "report",
      format: "json",
      displayName: `${"a".repeat(200)}.hprof`,
      mode: "deep",
      content: {},
    });
    expect(bounded.filename.length).toBeLessThanOrEqual(96);
  });

  it("normalizes content without interpreting markup and rejects unknown formats", async () => {
    const prepared = prepareExportDownload({
      kind: "flamegraph",
      format: "json",
      displayName: "fixture.hprof",
      mode: "deep",
      content: { title: "<script>alert(1)</script>", value: "a\u0000b\nc" },
    });

    expect(prepared.content).toContain("<script>alert(1)</script>");
    expect(prepared.content).not.toContain("\u0000");
    expect(JSON.parse(prepared.content)).toEqual({
      title: "<script>alert(1)</script>",
      value: "ab\nc",
    });
    expect(await prepared.blob.text()).toBe(prepared.content);

    expect(() =>
      prepareExportDownload({
        kind: "report",
        format: "exe" as never,
        displayName: "fixture.hprof",
        mode: "deep",
        content: "unsafe",
      }),
    ).toThrow(/unsupported report export format/i);
  });

  it("downloads through a temporary anchor and revokes the object URL", () => {
    const prepared = prepareExportDownload({
      kind: "report",
      format: "text",
      displayName: "fixture.hprof",
      mode: "deep",
      content: "report",
    });
    const createObjectURL = mock(() => "blob:download");
    const revokeObjectURL = mock(() => {});
    const click = mock(() => {});
    const remove = mock(() => {});
    const appendChild = mock(() => {});
    const anchor = {
      href: "",
      download: "",
      rel: "",
      click,
      remove,
    } as unknown as HTMLAnchorElement;

    downloadExport(prepared, {
      createObjectURL,
      revokeObjectURL,
      createAnchor: () => anchor,
      appendChild,
    });

    expect(anchor.href).toBe("blob:download");
    expect(anchor.download).toBe("fixture-report-deep.txt");
    expect(anchor.rel).toBe("noopener");
    expect(appendChild).toHaveBeenCalledWith(anchor);
    expect(click).toHaveBeenCalledTimes(1);
    expect(remove).toHaveBeenCalledTimes(1);
    expect(revokeObjectURL).toHaveBeenCalledWith("blob:download");
  });
});
