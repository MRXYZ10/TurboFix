
import { useCallback, useEffect, useRef, useState } from "react";
import type { Api } from "./api";
import { toApiError } from "./api";
import type {
  ApiError,
  PerfSnapshot,
  ScanProgress,
  ScanReport,
} from "./types";

export function useScan(
  api: Api,
  onFinished: (report: ScanReport) => void,
) {
  const [running, setRunning] = useState(false);
  const [progress, setProgress] = useState<ScanProgress | null>(null);
  const [error, setError] = useState<ApiError | null>(null);
  const [cancelling, setCancelling] = useState(false);

  const runningRef = useRef(false);

  const start = useCallback(async (): Promise<boolean> => {
    // Prevent duplicate scans.
    if (runningRef.current) {
      return false;
    }

    runningRef.current = true;
    setRunning(true);
    setError(null);
    setProgress(null);
    setCancelling(false);

    let unlisten: (() => void) | undefined;

    try {
      // Listen for scan progress updates.
      unlisten = await api
        .onScanProgress(setProgress)
        .catch(() => undefined);

      // Run the scan and update the latest report.
      const report = await api.startScan();
      onFinished(report);

      return true;
    } catch (e) {
      setError(toApiError(e));
      return false;
    } finally {
      unlisten?.();
      runningRef.current = false;
      setRunning(false);
      setCancelling(false);
    }
  }, [api, onFinished]);

  const cancel = useCallback(async () => {
    if (!runningRef.current) {
      return;
    }

    setCancelling(true);

    try {
      await api.cancelScan();
    } catch (e) {
      setError(toApiError(e));
    }
  }, [api]);

  return {
    running,
    progress,
    error,
    cancelling,
    start,
    cancel,
  };
}

export function usePerf(
  api: Api,
  intervalMs = 2000,
  keep = 60,
) {
  const [snapshot, setSnapshot] = useState<PerfSnapshot | null>(null);
  const [cpuHistory, setCpuHistory] = useState<number[]>([]);
  const [memHistory, setMemHistory] = useState<number[]>([]);
  const [error, setError] = useState<ApiError | null>(null);

  useEffect(() => {
    let stopped = false;
    let timer: ReturnType<typeof setTimeout> | undefined;

    const tick = async () => {
      if (stopped) {
        return;
      }

      if (!document.hidden) {
        try {
          const s = await api.samplePerformance();

          if (stopped) {
            return;
          }

          setSnapshot(s);
          setError(null);

          setCpuHistory((history) =>
            [...history, s.cpu_pct].slice(-keep),
          );

          const memPct =
            s.memory_total_bytes > 0
              ? (s.memory_used_bytes / s.memory_total_bytes) * 100
              : 0;

          setMemHistory((history) =>
            [...history, memPct].slice(-keep),
          );
        } catch (e) {
          if (!stopped) {
            setError(toApiError(e));
          }
        }
      }

      if (!stopped) {
        timer = setTimeout(tick, intervalMs);
      }
    };

    void tick();

    return () => {
      stopped = true;

      if (timer !== undefined) {
        clearTimeout(timer);
      }
    };
  }, [api, intervalMs, keep]);

  return {
    snapshot,
    cpuHistory,
    memHistory,
    error,
  };
}
