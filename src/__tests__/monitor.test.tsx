import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import PingMonitor from "../pages/PingMonitor";
import type { MonitorSnapshot } from "../types";
import { mockApi } from "./helpers";

function runningSnapshot(): MonitorSnapshot {
  return {
    id: null, config: { targets: ["127.0.0.1:443"], interval_ms: 500, timeout_ms: 1000, max_concurrency: 1 },
    started_at: "2026-10-14T10:00:00Z", elapsed_ms: 1000, running: true, total_attempts: 1,
    successful_attempts: 0, failed_attempts: 1, last_updated_at: "2026-10-14T10:00:01Z",
    targets: [{ target: "127.0.0.1:443", attempts: 1, successful_attempts: 0, failed_attempts: 1, connection_refusals: 1,
      resolution_failures: 0, resolution_timeouts: 0, connection_timeouts: 0, transport_errors: 0,
      latest_success_latency_ms: null, latest_success_at: null, jitter_ms: null,
      last_result: { at: "2026-10-14T10:00:01Z", outcome: { outcome: "connection_refused", latency_ms: 1.2 } },
      recent_results: [{ at: "2026-10-14T10:00:01Z", outcome: { outcome: "connection_refused", latency_ms: 1.2 } }], latency_history: [] }],
  };
}

describe("Pro Network Monitor", () => {
  it("starts and stops the Rust monitoring session and shows per-target results", async () => {
    const onNetworkMonitorUpdate = vi.fn().mockResolvedValue(() => undefined);
    const startNetworkMonitor = vi.fn().mockResolvedValue(runningSnapshot());
    const stopNetworkMonitor = vi.fn().mockResolvedValue(true);
    const api = mockApi({ onNetworkMonitorUpdate, startNetworkMonitor, stopNetworkMonitor, getNetworkMonitorSnapshot: vi.fn().mockResolvedValue(null) });
    render(<PingMonitor api={api} />);
    await userEvent.click(await screen.findByRole("button", { name: "Start Monitoring" }));
    await waitFor(() => expect(startNetworkMonitor).toHaveBeenCalledWith(expect.objectContaining({ targets: ["1.1.1.1:443", "8.8.8.8:443"] })));
    expect(await screen.findByText("Port refused connection")).toBeInTheDocument();
    expect(screen.getByText("IP packet loss: Not measured")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Stop Monitoring" }));
    await waitFor(() => expect(stopNetworkMonitor).toHaveBeenCalledTimes(1));
  });

  it("shows backend start errors and remains restartable", async () => {
    const startNetworkMonitor = vi.fn().mockRejectedValue(new Error("network monitor is already running"));
    render(<PingMonitor api={mockApi({ startNetworkMonitor })} />);
    await userEvent.click(await screen.findByRole("button", { name: "Start Monitoring" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("already running");
    expect(screen.getByRole("button", { name: "Start Monitoring" })).toBeEnabled();
  });

  it("surfaces a backend stop or session-persistence error", async () => {
    const api = mockApi({
      startNetworkMonitor: vi.fn().mockResolvedValue(runningSnapshot()),
      stopNetworkMonitor: vi.fn().mockRejectedValue(new Error("could not save monitor session")),
      getNetworkMonitorSnapshot: vi.fn().mockResolvedValue({ ...runningSnapshot(), running: false }),
    });
    render(<PingMonitor api={api} />);
    await userEvent.click(await screen.findByRole("button", { name: "Start Monitoring" }));
    await userEvent.click(await screen.findByRole("button", { name: "Stop Monitoring" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("could not save monitor session");
    expect(screen.getByRole("button", { name: "Start Monitoring" })).toBeInTheDocument();
  });
});
