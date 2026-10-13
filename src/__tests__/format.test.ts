import { describe, expect, it } from "vitest";
import { fmtBytes, fmtMs, fmtPct, fmtRate, median } from "../lib/format";

describe("format helpers", () => {
  it("shows Unavailable for missing values instead of zero", () => {
    expect(fmtMs(null)).toBe("Unavailable");
    expect(fmtMs(undefined)).toBe("Unavailable");
    expect(fmtMs(NaN)).toBe("Unavailable");
    expect(fmtPct(null)).toBe("Unavailable");
    expect(fmtBytes(null)).toBe("Unavailable");
    expect(fmtRate(null)).toBe("Unavailable");
  });
  it("formats real values", () => {
    expect(fmtMs(5.25)).toBe("5.3 ms");
    expect(fmtMs(123.4)).toBe("123 ms");
    expect(fmtPct(12.34, 1)).toBe("12.3%");
    expect(fmtBytes(1536)).toBe("1.5 KB");
    expect(fmtBytes(0)).toBe("0 B");
    expect(fmtRate(2048)).toBe("2.0 KB/s");
  });
  it("median ignores nulls", () => {
    expect(median([null, null])).toBeNull();
    expect(median([3, null, 1, 2])).toBe(2);
    expect(median([1, 2, 3, 4])).toBe(2.5);
  });
});
