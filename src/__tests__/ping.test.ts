import { describe, expect, it } from "vitest";
import { summarizePings } from "../lib/ping";

describe("summarizePings", () => {
  it("summarizes real successful samples and counts failures", () => {
    expect(summarizePings([10, null, 14, 12])).toEqual({ averageMs: 12, jitterMs: 3, failed: 1 });
  });

  it("does not invent latency when every probe fails", () => {
    expect(summarizePings([null, null])).toEqual({ averageMs: null, jitterMs: null, failed: 2 });
  });

  it("does not report jitter from a single successful sample", () => {
    expect(summarizePings([null, 8])).toEqual({ averageMs: 8, jitterMs: null, failed: 1 });
  });
});
