import type { ReactNode } from "react";
import type { Confidence, OverallStatus, Severity } from "../types";

export function Card({ title, children, right }: { title?: string; children: ReactNode; right?: ReactNode }) {
  return (
    <section className="rounded-xl border border-ink-700 bg-ink-800/70 p-4 shadow-sm">
      {(title || right) && (
        <header className="mb-3 flex items-center justify-between">
          {title && <h2 className="text-sm font-semibold uppercase tracking-wide text-slate-400">{title}</h2>}
          {right}
        </header>
      )}
      {children}
    </section>
  );
}

export function Metric({ label, value, hint, unavailable }: { label: string; value: string; hint?: string; unavailable?: boolean }) {
  return (
    <div className="rounded-lg border border-ink-700 bg-ink-900/60 p-3" data-testid={`metric-${label}`}>
      <div className="text-xs text-slate-400">{label}</div>
      <div className={`mt-1 text-xl font-semibold ${unavailable ? "text-slate-500" : "text-slate-100"}`}>{value}</div>
      {hint && <div className="mt-1 text-xs text-slate-500">{hint}</div>}
    </div>
  );
}

const statusStyle: Record<OverallStatus, string> = {
  online: "bg-emerald-500/15 text-emerald-300 border-emerald-500/30",
  degraded: "bg-amber-500/15 text-amber-300 border-amber-500/30",
  offline: "bg-red-500/15 text-red-300 border-red-500/30",
  unknown: "bg-slate-500/15 text-slate-300 border-slate-500/30",
};
const statusLabel: Record<OverallStatus, string> = { online: "Online", degraded: "Issues found", offline: "Offline", unknown: "Unknown" };

export function StatusBadge({ status }: { status: OverallStatus | null }) {
  const s = status ?? "unknown";
  return <span className={`rounded-full border px-3 py-1 text-xs font-medium ${statusStyle[s]}`}>{status ? statusLabel[s] : "No scan yet"}</span>;
}

const sevStyle: Record<Severity, string> = { info: "text-sky-300", warning: "text-amber-300", critical: "text-red-300" };
export function SeverityDot({ severity }: { severity: Severity }) {
  return <span aria-label={severity} className={`mr-2 inline-block h-2 w-2 rounded-full bg-current ${sevStyle[severity]}`} />;
}

export function ConfidenceTag({ value }: { value: Confidence }) {
  return <span className="rounded border border-ink-600 px-1.5 py-0.5 text-[10px] uppercase tracking-wide text-slate-400">{value} confidence</span>;
}

export function ErrorBanner({ message, onRetry }: { message: string; onRetry?: () => void }) {
  return (
    <div role="alert" className="flex items-center justify-between rounded-lg border border-red-500/30 bg-red-500/10 p-3 text-sm text-red-200">
      <span>{message}</span>
      {onRetry && (
        <button onClick={onRetry} className="ml-4 rounded border border-red-400/40 px-2 py-1 text-xs hover:bg-red-500/20">
          Retry
        </button>
      )}
    </div>
  );
}

export function Sparkline({ values, max, label }: { values: number[]; max?: number; label: string }) {
  if (values.length < 2) return <div className="h-12 text-xs text-slate-500">Collecting samples…</div>;
  const hi = max ?? Math.max(...values, 1);
  const w = 240;
  const h = 48;
  const pts = values.map((v, i) => `${(i / (values.length - 1)) * w},${h - (Math.min(v, hi) / hi) * h}`).join(" ");
  return (
    <svg role="img" aria-label={label} viewBox={`0 0 ${w} ${h}`} className="h-12 w-full">
      <polyline points={pts} fill="none" stroke="#22d3ee" strokeWidth="2" strokeLinejoin="round" />
    </svg>
  );
}

export function Bar({ pct, label }: { pct: number; label: string }) {
  const p = Math.max(0, Math.min(100, pct));
  const color = p > 90 ? "bg-red-400" : p > 75 ? "bg-amber-400" : "bg-accent";
  return (
    <div role="progressbar" aria-label={label} aria-valuenow={Math.round(p)} aria-valuemin={0} aria-valuemax={100} className="h-2 w-full rounded bg-ink-700">
      <div className={`h-2 rounded ${color}`} style={{ width: `${p}%` }} />
    </div>
  );
}
