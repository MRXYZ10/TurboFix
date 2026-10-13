import { Bar, Card, ErrorBanner, Sparkline } from "../components/ui";
import { fmtBytes, fmtPct, fmtRate } from "../lib/format";
import type { ApiError, PerfSnapshot } from "../types";

interface Props {
  perf: { snapshot: PerfSnapshot | null; cpuHistory: number[]; memHistory: number[]; error: ApiError | null };
}

export default function Performance({ perf }: Props) {
  const s = perf.snapshot;
  return (
    <div className="space-y-4">
      <div>
        <h1 className="text-2xl font-semibold text-slate-100">PC Performance</h1>
        <p className="text-sm text-slate-400">Live system metrics, sampled every 2 seconds while this page is open. TurboFix never ends processes or changes services.</p>
      </div>
      {perf.error && <ErrorBanner message={perf.error.message} />}
      {!s && !perf.error && <p className="text-sm text-slate-400">Reading system metrics…</p>}
      {s && (
        <>
          <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
            <Card title={`CPU · ${s.cpu_count} logical cores`}>
              <div className="mb-1 text-2xl font-semibold">{fmtPct(s.cpu_pct, 0)}</div>
              <Sparkline values={perf.cpuHistory} max={100} label="CPU history" />
            </Card>
            <Card title="Memory">
              <div className="mb-1 text-2xl font-semibold">{fmtBytes(s.memory_used_bytes)} <span className="text-sm text-slate-400">of {fmtBytes(s.memory_total_bytes)}</span></div>
              <Sparkline values={perf.memHistory} max={100} label="Memory history" />
              <p className="text-xs text-slate-500">Available: {fmtBytes(s.memory_available_bytes)} · Swap/pagefile used: {fmtBytes(s.swap_used_bytes)} of {fmtBytes(s.swap_total_bytes)}</p>
            </Card>
          </div>

          <Card title="Network activity (all adapters)">
            <p className="text-sm">Receive {fmtRate(s.net_rx_bytes_per_s)} · Send {fmtRate(s.net_tx_bytes_per_s)}</p>
          </Card>

          <Card title="Disk capacity">
            <p className="mb-2 text-xs text-slate-500">Disk activity (throughput, queue length) is not measured in this version.</p>
            <ul className="space-y-3">
              {s.disks.map((d) => {
                const used = d.total_bytes - d.available_bytes;
                const pct = d.total_bytes ? (used / d.total_bytes) * 100 : 0;
                return (
                  <li key={d.mount + d.name}>
                    <div className="flex justify-between text-sm"><span>{d.mount} <span className="text-slate-500">{d.file_system}</span></span><span>{fmtBytes(used)} / {fmtBytes(d.total_bytes)}</span></div>
                    <Bar pct={pct} label={`Disk ${d.mount} used`} />
                  </li>
                );
              })}
            </ul>
          </Card>

          <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
            <Card title="Top processes by CPU">
              <p className="mb-2 text-xs text-slate-500">CPU % is relative to one core, so values above 100% are possible on multi-core PCs.</p>
              <table className="w-full text-left text-sm"><tbody>
                {s.top_cpu.map((p) => <tr key={p.pid} className="border-t border-ink-700"><td className="py-1">{p.name}</td><td className="text-slate-500">{p.pid}</td><td className="text-right">{p.cpu_pct.toFixed(1)}%</td></tr>)}
              </tbody></table>
            </Card>
            <Card title="Top processes by memory">
              <p className="mb-2 text-xs text-slate-500">{s.process_count} processes running. A large process is not necessarily a problem.</p>
              <table className="w-full text-left text-sm"><tbody>
                {s.top_memory.map((p) => <tr key={p.pid} className="border-t border-ink-700"><td className="py-1">{p.name}</td><td className="text-slate-500">{p.pid}</td><td className="text-right">{fmtBytes(p.memory_bytes)}</td></tr>)}
              </tbody></table>
            </Card>
          </div>
        </>
      )}
    </div>
  );
}
