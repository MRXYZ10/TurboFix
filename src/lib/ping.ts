export function summarizePings(samples: (number | null)[]) {
  const values = samples.filter((value): value is number => value !== null && Number.isFinite(value));
  const mean = values.length ? values.reduce((sum, value) => sum + value, 0) / values.length : null;
  const jitter = values.length > 1
    ? values.slice(1).reduce((sum, value, index) => sum + Math.abs(value - values[index]), 0) / (values.length - 1)
    : null;
  return { averageMs: mean, jitterMs: jitter, failed: samples.length - values.length };
}
