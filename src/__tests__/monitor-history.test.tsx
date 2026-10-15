import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import History from "../pages/History";
import { mockApi } from "./helpers";

it("shows monitor-only history and allows users to clear it explicitly", async () => {
  const clearHistory = vi.fn().mockResolvedValue(1);
  const api = mockApi({
    listScans: vi.fn().mockResolvedValue([]),
    listMonitorSessions: vi.fn().mockResolvedValue([{
      id: 4, started_at: "2026-10-14T10:00:00Z", ended_at: "2026-10-14T10:00:10Z",
      duration_ms: 10_000, interval_ms: 1000, timeout_ms: 2000, total_attempts: 8,
      successful_attempts: 7, failed_attempts: 1, targets: [{
        target: "127.0.0.1:443", attempts: 8, successful_attempts: 7, failed_attempts: 1,
        connection_refusals: 1, resolution_failures: 0, resolution_timeouts: 0,
        connection_timeouts: 0, transport_errors: 0, latest_success_latency_ms: 2,
        jitter_ms: 0.5, last_result: null,
      }],
    }]),
    clearHistory,
  });
  render(<History api={api} version={0} onOpen={vi.fn()} onChanged={vi.fn()} speedTests={[]} />);

  expect(await screen.findByText("Saved network monitor sessions")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Clear all history…" })).toBeInTheDocument();
  await userEvent.click(screen.getByRole("button", { name: "Clear all history…" }));
  expect(screen.getByRole("alertdialog")).toHaveTextContent("network monitor sessions");
  await userEvent.click(screen.getByRole("button", { name: "Delete everything" }));
  expect(clearHistory).toHaveBeenCalledTimes(1);
});
