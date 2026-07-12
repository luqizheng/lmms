import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import { downloadRaw, timestampedFilename } from "./download";

describe("downloadRaw", () => {
  let clickMock: ReturnType<typeof vi.fn>;
  let appendMock: ReturnType<typeof vi.fn>;
  let removeMock: ReturnType<typeof vi.fn>;
  let createObjectURLMock: ReturnType<typeof vi.fn>;
  let revokeObjectURLMock: ReturnType<typeof vi.fn>;
  let capturedAnchor: any;
  let capturedBlob: any;

  beforeEach(() => {
    clickMock = vi.fn();
    appendMock = vi.fn();
    removeMock = vi.fn();
    createObjectURLMock = vi.fn((b: any) => {
      capturedBlob = b;
      return "blob:mock-url";
    });
    revokeObjectURLMock = vi.fn();
    capturedAnchor = null;

    vi.stubGlobal("URL", {
      createObjectURL: createObjectURLMock,
      revokeObjectURL: revokeObjectURLMock
    });
    vi.stubGlobal("document", {
      createElement: (tag: string) => {
        if (tag === "a") {
          capturedAnchor = { href: "", download: "", click: clickMock };
          return capturedAnchor;
        }
        return {};
      },
      body: { appendChild: appendMock, removeChild: removeMock }
    });
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it("creates a Blob, anchors <a>, clicks, and revokes object URL", () => {
    downloadRaw("hello,world", "test.csv", "text/csv");

    expect(createObjectURLMock).toHaveBeenCalledTimes(1);
    expect(capturedBlob).toBeInstanceOf(Blob);
    expect((capturedBlob as Blob).type).toBe("text/csv");

    expect(capturedAnchor.href).toBe("blob:mock-url");
    expect(capturedAnchor.download).toBe("test.csv");
    expect(appendMock).toHaveBeenCalledWith(capturedAnchor);
    expect(clickMock).toHaveBeenCalledTimes(1);
    expect(removeMock).toHaveBeenCalledWith(capturedAnchor);
    expect(revokeObjectURLMock).toHaveBeenCalledWith("blob:mock-url");
  });
});

describe("timestampedFilename", () => {
  it("formats prefix + YYYYMMDD_HHMMSS + .ext", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-07-08T03:04:05"));
    const name = timestampedFilename("audit", "csv");
    expect(name).toBe("audit_20260708_030405.csv");
    vi.useRealTimers();
  });

  it("zero-pads single-digit months/days/hours/minutes/seconds", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-01-02T03:04:05"));
    const name = timestampedFilename("log", "json");
    expect(name).toBe("log_20260102_030405.json");
    vi.useRealTimers();
  });
});
