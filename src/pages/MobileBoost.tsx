import { useCallback, useEffect, useState } from "react";
import type { Api } from "../api";
import { mobileBoostApi } from "../api";
import type { MobileWifiInfo } from "../api";
import type { SpeedTestResult } from "../types";

type NumberRecord = Record<string, unknown>;

function resultNumber(result: SpeedTestResult | null, keys: string[]): number | null {
  if (!result) return null;
  const record = result as unknown as NumberRecord;
  for (const key of keys) {
    const value = record[key];
    if (typeof value === "number" && Number.isFinite(value)) return value;
  }
  return null;
}

function formatMbps(value: number | null): string {
  return value === null ? "Not measured" : `${value.toFixed(1)} Mbps`;
}

function formatRssi(value: number | null): string {
  return value === null ? "Not available" : `${value} dBm`;
}

function signalLabel(value: number | null): string {
  if (value === null) return "Unknown";
  if (value >= -55) return "Excellent";
  if (value >= -67) return "Good";
  if (value >= -75) return "Fair";
  return "Weak";
}

function wifiBand(value: number | null): string {
  if (value === null) return "Unknown";
  if (value >= 5925) return "6 GHz";
  if (value >= 4900) return "5 GHz";
  if (value >= 2400) return "2.4 GHz";
  return `${value} MHz`;
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export default function MobileBoost({ api }: { api: Api }) {
  const [wifi, setWifi] = useState<MobileWifiInfo | null>(null);
  const [speed, setSpeed] = useState<SpeedTestResult | null>(null);
  const [previousSpeed, setPreviousSpeed] = useState<SpeedTestResult | null>(null);
  const [loadingWifi, setLoadingWifi] = useState(false);
  const [runningCheck, setRunningCheck] = useState(false);
  const [openingSettings, setOpeningSettings] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const refreshWifi = useCallback(async () => {
    setLoadingWifi(true);
    setError(null);
    try {
      setWifi(await mobileBoostApi.wifiInfo());
    } catch (reason) {
      setError(errorMessage(reason));
    } finally {
      setLoadingWifi(false);
    }
  }, []);

  useEffect(() => {
    void refreshWifi();
  }, [refreshWifi]);

  const runBoostCheck = async () => {
    setRunningCheck(true);
    setError(null);
    setNotice(null);
    try {
      const currentWifi = await mobileBoostApi.wifiInfo();
      setWifi(currentWifi);
      const result = await api.runSpeedTest();
      setPreviousSpeed(speed);
      setSpeed(result);
      setNotice("Measurements complete. TurboFix has not changed Wi-Fi or router settings automatically.");
    } catch (reason) {
      setError(errorMessage(reason));
    } finally {
      setRunningCheck(false);
    }
  };

  const openSettings = async () => {
    setOpeningSettings(true);
    setError(null);
    try {
      await mobileBoostApi.openWifiSettings();
    } catch (reason) {
      setError(errorMessage(reason));
    } finally {
      setOpeningSettings(false);
    }
  };

  const download = resultNumber(speed, ["download_mbps", "downloadMbps"]);
  const upload = resultNumber(speed, ["upload_mbps", "uploadMbps"]);
  const latency = resultNumber(speed, ["latency_ms", "ping_ms", "rtt_ms", "latencyMs", "pingMs"]);
  const previousDownload = resultNumber(previousSpeed, ["download_mbps", "downloadMbps"]);
  const previousUpload = resultNumber(previousSpeed, ["upload_mbps", "uploadMbps"]);

  const currentRssi = wifi?.rssiDbm ?? null;
  let recommendation = "Run a boost check to measure this connection before making recommendations.";
  if (wifi && !wifi.wifiEnabled) {
    recommendation = "Wi-Fi is turned off. Open Android Wi-Fi settings and enable Wi-Fi or choose your network.";
  } else if (wifi && !wifi.connected) {
    recommendation = "No active Wi-Fi connection is detected. Connect to a Wi-Fi network in Android settings, then refresh.";
  } else if (currentRssi !== null && currentRssi <= -75) {
    recommendation = "The Wi-Fi signal is weak. Move closer to the router and reduce walls or metal obstacles, then run the test again.";
  } else if (currentRssi !== null && currentRssi <= -67) {
    recommendation = "Signal strength is fair. Try testing closer to the router; compare 2.4 GHz range with 5 GHz performance if your router supports both.";
  } else if (latency !== null && latency >= 100) {
    recommendation = "Signal does not look obviously weak, but measured latency is high. Repeat the test when large downloads or other streaming devices are idle.";
  } else if (wifi?.connected) {
    recommendation = "No obvious weak-signal issue was detected from the available reading. If internet speed is still low, check router congestion, your broadband plan, and repeat the speed test near the router.";
  }

  const metricClass = "rounded-xl border border-ink-700 bg-ink-800/50 p-4";

  return (
    <div className="space-y-4 mobile-boost-page">
      <section className="rounded-xl border border-ink-700 bg-ink-800/50 p-4 sm:p-5">
        <div className="flex flex-wrap items-start justify-between gap-4">
          <div className="min-w-0 flex-1">
            <div className="mb-2 text-xs font-bold uppercase tracking-[0.18em] text-cyan-300">Android network lab</div>
            <h2 className="text-2xl font-semibold text-slate-100">Connection quality</h2>
            <p className="mt-2 max-w-2xl text-sm leading-6 text-slate-400">
              Measure your phone's Wi-Fi signal and real internet throughput, then get practical steps for improving the connection.
            </p>
          </div>
          <div className={`rounded-full border px-3 py-1.5 text-xs font-semibold ${wifi?.connected ? "border-emerald-700 bg-emerald-950/50 text-emerald-300" : "border-amber-700 bg-amber-950/40 text-amber-200"}`}>
            {wifi?.connected ? "Wi-Fi connected" : wifi?.wifiEnabled ? "Not connected" : "Wi-Fi off / unknown"}
          </div>
        </div>
        <div className="mt-5 grid grid-cols-1 gap-3 sm:grid-cols-3">
          <div className={metricClass}>
            <div className="text-xs text-slate-400">Signal strength</div>
            <div className="mt-2 text-2xl font-semibold text-slate-100">{formatRssi(wifi?.rssiDbm ?? null)}</div>
            <div className="mt-1 text-xs text-cyan-300">{signalLabel(wifi?.rssiDbm ?? null)}</div>
          </div>
          <div className={metricClass}>
            <div className="text-xs text-slate-400">Receive link rate</div>
            <div className="mt-2 text-2xl font-semibold text-slate-100">{wifi?.rxLinkSpeedMbps ? `${wifi.rxLinkSpeedMbps} Mbps` : "Not available"}</div>
            <div className="mt-1 text-xs text-slate-500">Phone â†” router, not internet speed</div>
          </div>
          <div className={metricClass}>
            <div className="text-xs text-slate-400">Wi-Fi band</div>
            <div className="mt-2 text-2xl font-semibold text-slate-100">{wifiBand(wifi?.frequencyMhz ?? null)}</div>
            <div className="mt-1 text-xs text-slate-500">{wifi?.frequencyMhz ? `${wifi.frequencyMhz} MHz` : "Frequency not exposed"}</div>
          </div>
        </div>
        {wifi?.note && <p className="mt-4 text-xs leading-5 text-slate-400">{wifi.note}</p>}
        <div className="mt-5 flex flex-col gap-2 sm:flex-row">
          <button onClick={() => void runBoostCheck()} disabled={runningCheck || loadingWifi} className="rounded-lg bg-accent px-4 py-3 text-sm font-semibold text-slate-950 disabled:opacity-50">
            {runningCheck ? "Measuring connectionâ€¦" : "Run Mobile Boost Check"}
          </button>
          <button onClick={() => void refreshWifi()} disabled={loadingWifi || runningCheck} className="rounded-lg border border-ink-600 px-4 py-3 text-sm font-semibold text-slate-200 disabled:opacity-50">
            {loadingWifi ? "Refreshingâ€¦" : "Refresh Wi-Fi signal"}
          </button>
          <button onClick={() => void openSettings()} disabled={openingSettings} className="rounded-lg border border-ink-600 px-4 py-3 text-sm font-semibold text-slate-200 disabled:opacity-50">
            {openingSettings ? "Opening settingsâ€¦" : "Open Wi-Fi Settings"}
          </button>
        </div>
      </section>

      {error && <div role="alert" className="rounded-xl border border-rose-800 bg-rose-950/30 p-4 text-sm text-rose-200">{error}</div>}
      {notice && <div role="status" className="rounded-xl border border-emerald-800 bg-emerald-950/30 p-4 text-sm text-emerald-200">{notice}</div>}

      <section className="rounded-xl border border-ink-700 bg-ink-800/50 p-4 sm:p-5">
        <h3 className="text-base font-semibold text-slate-100">Internet speed test</h3>
        <p className="mt-1 text-sm text-slate-400">Results come from the app's real, bounded speed test. Wi-Fi link rate and internet throughput are different measurements.</p>
        <div className="mt-4 grid grid-cols-1 gap-3 sm:grid-cols-3">
          <div className={metricClass}>
            <div className="text-xs text-slate-400">Download</div>
            <div className="mt-2 text-xl font-semibold text-slate-100">{formatMbps(download)}</div>
            {previousDownload !== null && download !== null && <div className="mt-1 text-xs text-slate-500">Previous check: {formatMbps(previousDownload)} Â· Î” {(download - previousDownload).toFixed(1)} Mbps</div>}
          </div>
          <div className={metricClass}>
            <div className="text-xs text-slate-400">Upload</div>
            <div className="mt-2 text-xl font-semibold text-slate-100">{formatMbps(upload)}</div>
            {previousUpload !== null && upload !== null && <div className="mt-1 text-xs text-slate-500">Previous check: {formatMbps(previousUpload)} Â· Î” {(upload - previousUpload).toFixed(1)} Mbps</div>}
          </div>
          <div className={metricClass}>
            <div className="text-xs text-slate-400">Latency</div>
            <div className="mt-2 text-xl font-semibold text-slate-100">{latency === null ? "Not measured" : `${latency.toFixed(1)} ms`}</div>
          </div>
        </div>
      </section>

      <section className="rounded-xl border border-ink-700 bg-ink-800/50 p-4 sm:p-5">
        <h3 className="text-base font-semibold text-slate-100">Recommended next step</h3>
        <p className="mt-2 text-sm leading-6 text-slate-300">{recommendation}</p>
        <div className="mt-4 rounded-lg border border-ink-700/80 bg-ink-900/50 p-3 text-xs leading-5 text-slate-500">
          Android does not give ordinary apps a supported way to silently increase router power or force Wi-Fi toggles. TurboFix reads the data Android exposes and opens system Wi-Fi settings when you choose; it will not claim a speed boost unless measurements show one.
        </div>
      </section>
    </div>
  );
}