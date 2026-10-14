
import { useCallback, useEffect, useState } from "react";
import type { Api } from "../api";
import { Bar, Card, ErrorBanner, Metric } from "../components/ui";
import { fmtTime } from "../lib/format";
import type { ApiError, WifiSnapshot } from "../types";
import MobileBoost from "./MobileBoost";

function DesktopWifi({ api }: { api: Api }) {
  const [snapshot, setSnapshot] = useState<WifiSnapshot | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<ApiError | null>(null);

  const refresh = useCallback(async () => {
    if (loading) return;

    setLoading(true);
    setError(null);

    try {
      const result = await api.wifiSnapshot();
      setSnapshot(result);
    } catch (e) {
      const message =
        e && typeof e === "object" && "message" in e
          ? String((e as { message: unknown }).message)
          : String(e);

      setError({
        code: "wifi_unavailable",
        message,
      });
    } finally {
      setLoading(false);
    }
  }, [api, loading]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return (
    <div className="space-y-4">
      {/* Header */}
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h1 className="text-2xl font-semibold text-slate-100">
            Wi-Fi Analysis
          </h1>
          <p className="text-sm text-slate-400">
            Live Windows Wi-Fi adapter and nearby-network information.
          </p>
        </div>

        <button
          onClick={() => void refresh()}
          disabled={loading}
          className="rounded-lg border border-ink-600 px-4 py-2 text-sm transition-colors hover:bg-ink-700 disabled:cursor-not-allowed disabled:opacity-50"
        >
          {loading ? "Scanning…" : "Refresh Wi-Fi scan"}
        </button>
      </div>

      {/* Error and retry */}
      {error && (
        <ErrorBanner message={error.message} onRetry={() => void refresh()} />
      )}

      {/* Initial loading skeleton */}
      {!snapshot && loading && (
        <div
          role="status"
          aria-live="polite"
          className="space-y-4"
        >
          <div className="flex items-center gap-3 rounded-xl border border-ink-700 bg-ink-800/50 p-4">
            <div className="h-5 w-5 animate-spin rounded-full border-2 border-ink-600 border-t-cyan-400" />
            <div>
              <p className="text-sm font-medium text-slate-200">
                Reading Windows Wi-Fi information…
              </p>
              <p className="mt-1 text-xs text-slate-400">
                Detecting your connection and nearby networks.
              </p>
            </div>
          </div>

          <Card title="Current connection">
            <div className="animate-pulse space-y-5">
              <div className="h-6 w-48 rounded bg-ink-600" />

              <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-4">
                {["signal", "channel", "receive", "transmit"].map((item) => (
                  <div
                    key={item}
                    className="space-y-3 rounded-lg border border-ink-700 p-4"
                  >
                    <div className="h-3 w-24 rounded bg-ink-600" />
                    <div className="h-7 w-28 rounded bg-ink-600" />
                    <div className="h-3 w-32 rounded bg-ink-700" />
                  </div>
                ))}
              </div>

              <div className="h-2 w-full rounded bg-ink-700" />
            </div>
          </Card>

          <Card title="Nearby networks">
            <div className="animate-pulse space-y-3">
              <div className="h-4 w-40 rounded bg-ink-600" />
              <div className="h-10 w-full rounded bg-ink-700" />
              <div className="h-10 w-full rounded bg-ink-700" />
            </div>
          </Card>
        </div>
      )}

      {/* Unsupported platform */}
      {snapshot && !snapshot.supported && (
        <Card title="Wi-Fi information unavailable">
          <p className="text-sm text-slate-200">
            Wi-Fi adapter details are available on Windows 10/11.
          </p>
          <p className="mt-2 text-sm text-slate-400">{snapshot.note}</p>
        </Card>
      )}

      {/* Current connection and metrics */}
      {snapshot?.supported && (
        <>
          <Card
            title="Current connection"
            right={
              <span
                className={`rounded-full border px-3 py-1 text-xs ${
                  snapshot.connected
                    ? "border-emerald-500/30 bg-emerald-500/10 text-emerald-300"
                    : "border-amber-500/30 bg-amber-500/10 text-amber-300"
                }`}
              >
                {snapshot.connected ? "Connected" : "Not connected"}
              </span>
            }
          >
            <div className="mb-3 flex items-center justify-between gap-2">
              <div>
                <div className="text-xl font-semibold text-slate-100">
                  {snapshot.ssid ??
                    (snapshot.connected
                      ? "Wi-Fi network"
                      : "No active Wi-Fi link")}
                </div>
                <p className="text-xs text-slate-500">
                  Updated {fmtTime(snapshot.checked_at)}
                  {loading ? " · Refreshing…" : ""}
                </p>
              </div>

              <span className="text-2xl" aria-hidden="true">
                ⌁
              </span>
            </div>

            <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-4">
              <Metric
                label="Signal strength"
                value={
                  snapshot.signal_percent == null
                    ? "Unavailable"
                    : `${snapshot.signal_percent}%`
                }
                hint="Signal quality reported by Windows"
                unavailable={snapshot.signal_percent == null}
              />

              <Metric
                label="Channel"
                value={
                  snapshot.channel == null
                    ? "Unavailable"
                    : String(snapshot.channel)
                }
                hint={snapshot.band_hint ?? "Band unknown"}
                unavailable={snapshot.channel == null}
              />

              <Metric
                label="Receive link rate"
                value={
                  snapshot.receive_mbps == null
                    ? "Unavailable"
                    : `${snapshot.receive_mbps.toFixed(0)} Mbps`
                }
                hint="PC ← router, not internet speed"
                unavailable={snapshot.receive_mbps == null}
              />

              <Metric
                label="Transmit link rate"
                value={
                  snapshot.transmit_mbps == null
                    ? "Unavailable"
                    : `${snapshot.transmit_mbps.toFixed(0)} Mbps`
                }
                hint="PC → router, not internet speed"
                unavailable={snapshot.transmit_mbps == null}
              />
            </div>

            {snapshot.signal_percent != null && (
              <div className="mt-4">
                <div className="mb-1 flex justify-between text-xs text-slate-400">
                  <span>Reported signal</span>
                  <span>{snapshot.signal_percent}%</span>
                </div>

                <Bar
                  pct={Math.max(
                    0,
                    Math.min(100, snapshot.signal_percent)
                  )}
                  label="Wi-Fi signal strength"
                />
              </div>
            )}

            <p className="mt-3 text-xs text-slate-500">
              Radio: {snapshot.radio_type ?? "Unavailable"}. {snapshot.note}
            </p>
          </Card>

          {/* Nearby networks */}
          <Card
            title={`Nearby networks (${snapshot.nearby_networks.length} shown)`}
          >
            {snapshot.nearby_networks.length === 0 ? (
              <p className="text-sm text-slate-500">
                Windows did not return nearby networks. The adapter may be
                disabled or scanning may be unavailable.
              </p>
            ) : (
              <div className="overflow-x-auto">
                <table className="w-full text-left text-sm">
                  <thead className="text-xs text-slate-500">
                    <tr>
                      <th className="py-2 pr-3">Network name</th>
                      <th className="pr-3">Signal</th>
                      <th className="pr-3">Channel</th>
                      <th>Radio</th>
                    </tr>
                  </thead>

                  <tbody>
                    {snapshot.nearby_networks.map((network, index) => (
                      <tr
                        key={`${network.ssid}-${index}`}
                        className="border-t border-ink-700"
                      >
                        <td className="py-2 pr-3 font-medium text-slate-200">
                          {network.ssid}
                        </td>

                        <td className="pr-3">
                          {network.signal_percent == null
                            ? "—"
                            : `${network.signal_percent}%`}
                        </td>

                        <td className="pr-3">
                          {network.channel ?? "—"}
                        </td>

                        <td>{network.radio_type ?? "—"}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}

            <p className="mt-3 text-xs text-slate-500">
              Only a quick signal snapshot, not a full RF spectrum analyser.
              SSIDs are not saved in history or sent by TurboFix.
            </p>
          </Card>

          {/* Improvement tips */}
          <Card title="How to improve a weak connection">
            <ol className="list-decimal space-y-2 pl-5 text-sm text-slate-300">
              <li>
                Move closer to the router and re-scan. Walls and distance can
                reduce signal.
              </li>

              <li>
                If both bands are offered, try 5 GHz for higher nearby
                throughput; 2.4 GHz may work better through walls. Actual
                performance depends on congestion and hardware.
              </li>

              <li>
                For gaming, compare the internet speed test and diagnostics
                while no large downloads are running.
              </li>

              <li>
                If signal stays poor beside the router, update the Wi-Fi
                adapter driver from your PC manufacturer&apos;s official
                support page.
              </li>
            </ol>
          </Card>
        </>
      )}

      {/* Recovery state if the first scan fails */}
      {!snapshot && !loading && error && (
        <Card title="Wi-Fi scan could not be completed">
          <p className="text-sm text-slate-400">
            Your Wi-Fi details could not be loaded. Check that your adapter is
            enabled, then retry the scan.
          </p>

          <button
            onClick={() => void refresh()}
            disabled={loading}
            className="mt-4 rounded-lg bg-cyan-500 px-4 py-2 text-sm font-medium text-slate-950 transition-colors hover:bg-cyan-400 disabled:opacity-50"
          >
            Try again
          </button>
        </Card>
      )}
    </div>
  );
}

export default function Wifi({ api }: { api: Api }) {
  const isAndroid = typeof navigator !== "undefined" && /Android/i.test(navigator.userAgent);
  if (isAndroid) return <MobileBoost api={api} />;
  return <DesktopWifi api={api} />;
}