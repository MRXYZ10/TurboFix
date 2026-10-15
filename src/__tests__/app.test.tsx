import { render, screen, waitFor, within } from "@testing-library/react";

import userEvent from "@testing-library/user-event";

import { describe, expect, it, vi } from "vitest";

import App from "../App";

import { mockApi, report, summary, cleanupPreview, speedTest } from "./helpers";

describe("Overview", () => {

  it("displays the version from package build metadata", async () => {
    render(<App api={mockApi()} />);
    expect(await screen.findByText("v0.3.4")).toBeInTheDocument();
  });

  it("reports failed startup initialization and supports retry", async () => {
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const getConfig = vi.fn()
      .mockRejectedValueOnce({ code: "storage_unavailable", message: "database token=secret-value unavailable" })
      .mockResolvedValueOnce(undefined);
    render(<App api={mockApi({ getConfig })} />);

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "TurboFix could not finish startup initialization (error storage_unavailable)",
    );
    expect(consoleError).toHaveBeenCalledWith("TurboFix startup initialization failed", {
      code: "storage_unavailable",
      message: "database token=[redacted] unavailable",
    });
    await userEvent.click(screen.getByRole("button", { name: "Retry" }));
    await waitFor(() => expect(getConfig).toHaveBeenCalledTimes(2));
    expect(screen.queryByText(/could not finish startup initialization/)).not.toBeInTheDocument();
    consoleError.mockRestore();
  });

  it("labels every network metric unavailable before any scan, never zero", async () => {

    render(<App api={mockApi()} />);

    expect(await screen.findByText(/No scan has been run yet/)).toBeInTheDocument();

    for (const label of ["Latency (median)", "Jitter", "TCP connect failures", "DNS (median)"]) {

     expect(

  within(screen.getByTestId("metric-" + label))

    .getByText(label === "TCP connect failures" ? "Not measured" : "Unavailable")

).toBeInTheDocument();

    }

    const dl = screen.getByTestId("metric-Download / upload");

    expect(within(dl).getByText("Not tested")).toBeInTheDocument();

    expect(within(dl).getByText(/Run a measured speed test/)).toBeInTheDocument();

  });

  it("shows real measured values from the latest saved scan", async () => {

    const api = mockApi({ listScans: vi.fn().mockResolvedValue([summary(1)]) });

    render(<App api={api} />);

    expect(await screen.findByText("21 ms")).toBeInTheDocument();

    expect(screen.getByText("No problems detected in these tests")).toBeInTheDocument();

    expect(screen.getByText("Online")).toBeInTheDocument();

  });

  it("shows live CPU/RAM from the backend", async () => {

    render(<App api={mockApi()} />);

    expect(await screen.findByLabelText("CPU usage")).toHaveAttribute("aria-valuenow", "12");

    expect(screen.getByLabelText("Memory usage")).toHaveAttribute("aria-valuenow", "50");

  });

  it("runs a scan, shows progress, then the result", async () => {

    let resolveScan: (r: ReturnType<typeof report>) => void = () => undefined;

    const startScan = vi.fn().mockImplementation(

      () => new Promise((res) => { resolveScan = res; })

    );

    render(<App api={mockApi({ startScan })} />);

    await userEvent.click(await screen.findByRole("button", { name: "Run diagnostic scan" }));

    expect(startScan).toHaveBeenCalledTimes(1);

    expect(await screen.findByRole("status")).toBeInTheDocument();

    expect(screen.getByRole("button", { name: "Scanning…" })).toBeDisabled();

    resolveScan(report());

    expect(await screen.findByText("21 ms")).toBeInTheDocument();

    expect(screen.queryByRole("status")).not.toBeInTheDocument();

  });

  it("can cancel a running scan", async () => {

    const cancelScan = vi.fn().mockResolvedValue(true);

    const startScan = vi.fn().mockImplementation(() => new Promise(() => undefined));

    render(<App api={mockApi({ startScan, cancelScan })} />);

    await userEvent.click(await screen.findByRole("button", { name: "Run diagnostic scan" }));

    await userEvent.click(await screen.findByRole("button", { name: "Cancel scan" }));

    expect(cancelScan).toHaveBeenCalledTimes(1);

  });

  it("surfaces scan errors and allows retry", async () => {

    const startScan = vi.fn()

      .mockRejectedValueOnce({

        code: "invalid_config",

        message: "invalid configuration: probe_count",

      })

      .mockResolvedValueOnce(report());

    render(<App api={mockApi({ startScan })} />);

    await userEvent.click(await screen.findByRole("button", { name: "Run diagnostic scan" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("probe_count");

    await userEvent.click(screen.getByRole("button", { name: "Retry" }));

    expect(await screen.findByText("21 ms")).toBeInTheDocument();

  });

  it("explains a cancelled scan and shows no diagnosis", async () => {

    const startScan = vi.fn().mockResolvedValue(

      report({ status: "cancelled", diagnosis: null, latency: [], dns: [] })

    );

    render(<App api={mockApi({ startScan })} />);

    await userEvent.click(await screen.findByRole("button", { name: "Run diagnostic scan" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(/cancelled/);

    expect(

      within(screen.getByTestId("metric-Latency (median)")).getByText("Unavailable")

    ).toBeInTheDocument();

  });

  it("reports an unavailable backend honestly", async () => {

    const listScans = vi.fn().mockRejectedValue({

      code: "no_backend",

      message: "TurboFix backend is not available.",

    });

    const samplePerformance = vi.fn().mockRejectedValue({

      code: "no_backend",

      message: "TurboFix backend is not available.",

    });

    render(<App api={mockApi({ listScans, samplePerformance })} />);

    expect((await screen.findAllByRole("alert")).length).toBeGreaterThan(0);

  });

});

describe("Navigation and honesty", () => {

  it("shows a preview and requires confirmation before temp-file deletion", async () => {

    const deleteTempFiles = vi.fn().mockResolvedValue({

      deleted_count: 1,

      deleted_bytes: 1024,

      skipped_count: 0,

      completed_at: "2026-01-01T00:00:00Z",

      message: "Done.",

    });

    render(

      <App

        api={mockApi({

          previewTempCleanup: vi.fn().mockResolvedValue(cleanupPreview()),

          deleteTempFiles,

        })}

      />

    );

    await userEvent.click(

      await screen.findByRole("button", { name: /Storage cleanup/i })

    );

    await userEvent.click(await screen.findByRole("button", { name: "Scan temp files" }));

    expect(await screen.findByText("old-cache.tmp")).toBeInTheDocument();

    await userEvent.click(screen.getByRole("checkbox", { name: "Select old-cache.tmp" }));

    await userEvent.click(screen.getByRole("button", { name: "Delete selected…" }));

    expect(deleteTempFiles).not.toHaveBeenCalled();

    await userEvent.click(screen.getByRole("button", { name: "Confirm delete" }));

    await waitFor(() => expect(deleteTempFiles).toHaveBeenCalledWith(["temp-000"]));

    expect(await screen.findByText(/Cleanup finished/)).toBeInTheDocument();

  });

  it("previews evidence and requires confirmation before execution and verification", async () => {
    const baseline = report({ diagnosis: { status: "degraded", findings: [{ id: "dns_partial", severity: "warning", confidence: "medium", title: "Some DNS lookups failed", detail: "Partial DNS failures", evidence: ["1/2 DNS lookups succeeded"] }], limitations: [] } });
    const executeOptimizerAction = vi.fn().mockResolvedValue({ result: { succeeded: true, title: "DNS cache flushed", message: "Windows completed the DNS cache flush.", completed_at: "2026-01-01T00:00:00Z" }, after: report(), comparisons: [{ metric: "median_dns_ms", before: 20, after: 19, delta: -1, result: "no measurable improvement" }], verification_error: null, history_id: 3 });
    render(<App api={mockApi({ startScan: vi.fn().mockResolvedValue(baseline), executeOptimizerAction })} />);

    await userEvent.click(

      await screen.findByRole("button", { name: /Smart optimizer/i })

    );

    await userEvent.click(

      screen.getByRole("button", { name: /Run scan for recommendations/ })

    );

    expect(await screen.findByText(/1\/2 DNS lookups succeeded/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: /Review DNS cache flush/ }));
    expect(executeOptimizerAction).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button", { name: /Confirm and apply/ }));
    await waitFor(() => expect(executeOptimizerAction).toHaveBeenCalledWith("flush_dns_cache", baseline.id));

    expect(await screen.findByText(/DNS cache flushed: Windows completed/)).toBeInTheDocument();
    expect(screen.getByText(/no measurable improvement/)).toBeInTheDocument();

  });

  it("reports an execution failure without claiming the action completed", async () => {
    const baseline = report({ diagnosis: { status: "degraded", findings: [{ id: "dns_partial", severity: "warning", confidence: "medium", title: "Some DNS lookups failed", detail: "Partial DNS failures", evidence: ["one lookup failed"] }], limitations: [] } });
    const executeOptimizerAction = vi.fn().mockResolvedValue({ result: { succeeded: false, title: "DNS cache flush failed", message: "Access denied.", completed_at: "2026-01-01T00:00:00Z" }, after: null, comparisons: [], verification_error: null, history_id: 4 });
    render(<App api={mockApi({ startScan: vi.fn().mockResolvedValue(baseline), executeOptimizerAction })} />);
    await userEvent.click(await screen.findByRole("button", { name: /Smart optimizer/i }));
    await userEvent.click(await screen.findByRole("button", { name: /Run scan for recommendations/ }));
    await userEvent.click(await screen.findByRole("button", { name: /Review DNS cache flush/ }));
    await userEvent.click(screen.getByRole("button", { name: /Confirm and apply/ }));
    expect(await screen.findByText(/action failed; verification was not run/)).toBeInTheDocument();
  });

  it("collapses the sidebar", async () => {

    render(<App api={mockApi()} />);

    await userEvent.click(await screen.findByRole("button", { name: "Collapse sidebar" }));

    expect(screen.getByRole("button", { name: "Expand sidebar" })).toBeInTheDocument();

  });

});

describe("History", () => {

  it("lists scans and requires confirmation before deleting one", async () => {

    const deleteScan = vi.fn().mockResolvedValue(undefined);

    const listScans = vi.fn().mockResolvedValue([summary(7)]);

    render(<App api={mockApi({ listScans, deleteScan })} />);

    await userEvent.click(

      await screen.findByRole("button", { name: /Reports & history/i })

    );

    await userEvent.click(await screen.findByRole("button", { name: "Delete" }));

    expect(deleteScan).not.toHaveBeenCalled();

    await userEvent.click(screen.getByRole("button", { name: "Cancel" }));

    expect(deleteScan).not.toHaveBeenCalled();

    await userEvent.click(screen.getByRole("button", { name: "Delete" }));

    await userEvent.click(screen.getByRole("button", { name: "Yes, delete" }));

    await waitFor(() => expect(deleteScan).toHaveBeenCalledWith(7));

  });

  it("requires confirmation before clearing all history", async () => {

    const clearHistory = vi.fn().mockResolvedValue(3);

    render(

      <App

        api={mockApi({

          listScans: vi.fn().mockResolvedValue([summary(1)]),

          clearHistory,

        })}

      />

    );

    await userEvent.click(

      await screen.findByRole("button", { name: /Reports & history/i })

    );

    await userEvent.click(screen.getByRole("button", { name: "Clear all history…" }));

    expect(screen.getByRole("alertdialog")).toBeInTheDocument();

    expect(clearHistory).not.toHaveBeenCalled();

    await userEvent.click(screen.getByRole("button", { name: "Keep" }));

    expect(clearHistory).not.toHaveBeenCalled();

    await userEvent.click(screen.getByRole("button", { name: "Clear all history…" }));

    await userEvent.click(screen.getByRole("button", { name: "Delete everything" }));

    await waitFor(() => expect(clearHistory).toHaveBeenCalledTimes(1));

  });

  it("opens a saved scan in diagnostics", async () => {

    const getScan = vi.fn().mockResolvedValue(report({ id: 7 }));

    render(

      <App

        api={mockApi({

          listScans: vi.fn().mockResolvedValue([summary(7)]),

          getScan,

        })}

      />

    );

    await userEvent.click(

      await screen.findByRole("button", { name: /Reports & history/i })

    );

    await userEvent.click(await screen.findByRole("button", { name: "Open" }));

    expect(

      await screen.findByText("TCP-connect latency and outcomes")

    ).toBeInTheDocument();

    expect(getScan).toHaveBeenCalledWith(7);

  });

});

describe("Settings", () => {

  it("loads and saves bounded connectivity probe settings", async () => {

    const saveConfig = vi.fn().mockResolvedValue(undefined);

    render(<App api={mockApi({ saveConfig })} />);

    await userEvent.click(await screen.findByRole("button", { name: "Settings" }));

    const count = await screen.findByRole("spinbutton", { name: "TCP probes per target" });

    const interval = screen.getByRole("spinbutton", {

      name: "Interval between probes (ms)",

    });

    await userEvent.clear(count);

    await userEvent.type(count, "25");

    await userEvent.clear(interval);

    await userEvent.type(interval, "350");

    await userEvent.click(screen.getByRole("button", { name: "Save settings" }));

    await waitFor(() =>

      expect(saveConfig).toHaveBeenCalledWith(

        expect.objectContaining({ probe_count: 25, probe_interval_ms: 350 })

      )

    );

    expect(await screen.findByText(/Settings saved/)).toBeInTheDocument();

  });

});

describe("Network tools", () => {
  it("runs a real speed-test command and shows the result", async () => {
    const runSpeedTest = vi.fn().mockResolvedValue({
      id: 3,
      started_at: "2026-01-01T00:00:00Z",
      duration_ms: 5000,
      download_mbps: 92.3,
      upload_mbps: 15.6,
      download_bytes: 5000000,
      upload_bytes: 1000000,
      download_duration_ms: 400,
      upload_duration_ms: 500,
      endpoint_label: "Cloudflare speed-test edge",
    });
    let resolveHistory: (results: ReturnType<typeof speedTest>[]) => void = () => undefined;
    const listSpeedTests = vi.fn().mockImplementation(
      () => new Promise<ReturnType<typeof speedTest>[]>((resolve) => { resolveHistory = resolve; })
    );

    render(<App api={mockApi({ runSpeedTest, listSpeedTests })} />);

    await userEvent.click(

      await screen.findByRole("button", { name: /^Speed test$/i })

    );

    await userEvent.click(

      screen.getByRole("button", { name: "Run speed test" })

    );

    expect(runSpeedTest).toHaveBeenCalledTimes(1);

    const downloadCard = screen
     .getByRole("heading", { name: /^Download$/ })
      .closest<HTMLElement>("section");
    if (!downloadCard) {
      throw new Error("Download result card was not rendered");
    }

    expect(await within(downloadCard).findByText("92.3 Mbps")).toBeInTheDocument();

    // Resolve the mount-time history request late; it must not wipe the fresh result.
    resolveHistory([]);
    await waitFor(() => {
      expect(within(downloadCard).getByText("92.3 Mbps")).toBeInTheDocument();
    });

  });

  it("loads Wi-Fi details from the Windows backend", async () => {

    render(<App api={mockApi()} />);

    await userEvent.click(

      await screen.findByRole("button", { name: /^Wi-Fi analysis$/i })

    );

    expect(

      (await screen.findAllByText("Lab Wi-Fi")).length

    ).toBeGreaterThan(0);

    expect(

      screen.getByText("82%", { selector: ".text-xl" })

    ).toBeInTheDocument();

  });

});
