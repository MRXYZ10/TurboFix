
import { useCallback, useEffect, useRef, useState } from "react";
import { api as realApi, toApiError } from "./api";
import type { Api } from "./api";
import { usePerf, useScan } from "./hooks";

import Diagnostics from "./pages/Diagnostics";
import History from "./pages/History";
import Overview from "./pages/Overview";
import Performance from "./pages/Performance";
import Cleanup from "./pages/Cleanup";
import SpeedTest from "./pages/SpeedTest";
import Wifi from "./pages/Wifi";
import Optimizer from "./pages/Optimizer";
import Settings from "./pages/Settings";

import type {
  ApiError,
  ScanReport,
  SpeedTestResult,
} from "./types";
import { ErrorBanner } from "./components/ui";

type PageId =
  | "overview"
  | "diagnostics"
  | "wifi"
  | "speedtest"
  | "performance"
  | "optimizer"
  | "cleanup"
  | "history"
  | "settings";

type NavItem = {
  id: PageId;
  label: string;
  icon: string;
  section: "Workspace" | "Tools" | "Manage";
  hint: string;
};

const NAV: NavItem[] = [
  {
    id: "overview",
    label: "Overview",
    icon: "â—«",
    section: "Workspace",
    hint: "Your network at a glance",
  },
  {
    id: "diagnostics",
    label: "Diagnostics",
    icon: "âŒ",
    section: "Workspace",
    hint: "Find the root cause",
  },
  {
    id: "speedtest",
    label: "Speed test",
    icon: "â†¯",
    section: "Workspace",
    hint: "Measure real throughput",
  },
  {
    id: "wifi",
    label: "Wi-Fi analysis",
    icon: "â—‰",
    section: "Tools",
    hint: "Inspect your wireless link",
  },
  {
    id: "performance",
    label: "PC performance",
    icon: "â–¥",
    section: "Tools",
    hint: "Live system health",
  },
  {
    id: "optimizer",
    label: "Smart optimizer",
    icon: "âœ³",
    section: "Tools",
    hint: "Safe, evidence-based fixes",
  },
  {
    id: "cleanup",
    label: "Storage cleanup",
    icon: "âŒ«",
    section: "Tools",
    hint: "Review temporary files",
  },
  {
    id: "history",
    label: "Reports & history",
    icon: "â–¤",
    section: "Manage",
    hint: "Your local measurements",
  },
  {
    id: "settings",
    label: "Settings",
    icon: "âš™",
    section: "Manage",
    hint: "Configure TurboFix",
  },
];

const pageTitle = (page: PageId) =>
  NAV.find((item) => item.id === page)?.label ?? "Overview";

export default function App({ api = realApi }: { api?: Api }) {
  const [page, setPage] = useState<PageId>("overview");
  const [collapsed, setCollapsed] = useState(false);
  const [mobileNav, setMobileNav] = useState(false);

  const [report, setReport] = useState<ScanReport | null>(null);
  const [version, setVersion] = useState(0);

  const [loadError, setLoadError] = useState<ApiError | null>(null);
  const [speedHistoryError, setSpeedHistoryError] =
    useState<ApiError | null>(null);

  const [speedHistory, setSpeedHistory] = useState<SpeedTestResult[]>([]);
  const [latestSpeedTest, setLatestSpeedTest] =
    useState<SpeedTestResult | null>(null);

  // Protect a newly completed test from older, in-flight history requests.
  const speedTestRevision = useRef(0);

  const loadLatest = useCallback(async () => {
    try {
      const list = await api.listScans(1);
      setReport(list.length ? await api.getScan(list[0].id) : null);
      setLoadError(null);
    } catch (e) {
      setLoadError(toApiError(e));
    }
  }, [api]);

  const loadSpeedHistory = useCallback(async () => {
    const revisionAtStart = speedTestRevision.current;

    try {
      const tests = await api.listSpeedTests(10);

      // A late history response must not replace a newer completed test.
      if (speedTestRevision.current === revisionAtStart) {
        setSpeedHistory(tests);
        setLatestSpeedTest(tests[0] ?? null);
      }

      setSpeedHistoryError(null);
    } catch (e) {
      setSpeedHistoryError(toApiError(e));
    }
  }, [api]);

  useEffect(() => {
    void loadLatest();
    void loadSpeedHistory();
  }, [api, loadLatest, loadSpeedHistory]);

  useEffect(() => {
    if (!mobileNav) return;

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setMobileNav(false);
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [mobileNav]);

  const onFinished = useCallback((result: ScanReport) => {
    setReport(result);
    setVersion((value) => value + 1);
  }, []);

  const scan = useScan(api, onFinished);
  const perf = usePerf(api, 2000);

  // Start a speed test and update its local history.
  const onSpeedTestFinished = useCallback((result: SpeedTestResult) => {
    speedTestRevision.current += 1;

    setLatestSpeedTest(result);
    setSpeedHistory((old) =>
      [result, ...old.filter((item) => item.id !== result.id)].slice(0, 10),
    );
  }, []);

  const reloadHistory = useCallback(() => {
    void loadLatest();
    void loadSpeedHistory();
  }, [loadLatest, loadSpeedHistory]);

  const navigate = (id: PageId) => {
    setPage(id);
    setMobileNav(false);
  };

  const open = async (id: number) => {
    try {
      setReport(await api.getScan(id));
      navigate("diagnostics");
    } catch (e) {
      setLoadError(toApiError(e));
    }
  };

  // Run verification and navigate only when the scan succeeds.
  const verifyDiagnostics = async () => {
    const success = await scan.start();

    if (success) {
      navigate("diagnostics");
    }
  };

  const groups = ["Workspace", "Tools", "Manage"] as const;

  return (
    <div className="app-shell">
      {mobileNav && (
        <button
          className="mobile-scrim"
          aria-label="Close navigation"
          onClick={() => setMobileNav(false)}
        />
      )}

      <aside
        className={`sidebar ${
          collapsed ? "sidebar-collapsed" : ""
        } ${mobileNav ? "sidebar-mobile-open" : ""}`}
      >
        <div className="brand-row">
          <div className="brand-mark" aria-hidden="true">
            <span>TF</span>
            <i />
          </div>

          {!collapsed && (
            <div className="brand-copy">
              <div className="brand-name">
                Turbo<span>Fix</span>
              </div>
              <div className="brand-caption">NETWORK INTELLIGENCE</div>
            </div>
          )}

          <button
            className="sidebar-toggle"
            aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
            onClick={() => setCollapsed((value) => !value)}
          >
            {collapsed ? "Â»" : "Â«"}
          </button>
        </div>

        <div className="workspace-pill">
          <span className="workspace-avatar">R</span>
          {!collapsed && (
            <>
              <span className="workspace-label">Personal workspace</span>
              <span className="workspace-chevron">âŒ„</span>
            </>
          )}
        </div>

        <nav aria-label="Main navigation" className="main-nav">
          {groups.map((group) => (
            <div className="nav-group" key={group}>
              {!collapsed && (
                <div className="nav-section-label">{group}</div>
              )}

              {NAV.filter((item) => item.section === group).map((item) => (
                <button
                  key={item.id}
                  onClick={() => navigate(item.id)}
                  aria-current={page === item.id ? "page" : undefined}
                  title={collapsed ? item.label : undefined}
                  className={`nav-item ${
                    page === item.id ? "nav-item-active" : ""
                  }`}
                >
                  <span className="nav-icon" aria-hidden="true">
                    {item.icon}
                  </span>

                  {!collapsed && (
                    <>
                      <span className="nav-label">{item.label}</span>
                      {item.id === "optimizer" && (
                        <span className="nav-dot" />
                      )}
                    </>
                  )}
                </button>
              ))}
            </div>
          ))}
        </nav>

        <div className="sidebar-bottom">
          <div className="privacy-card">
            <span className="privacy-icon">âŒ‘</span>
            {!collapsed && (
              <div>
                <strong>Private by design</strong>
                <p>Your reports stay on this device.</p>
              </div>
            )}
          </div>

          {!collapsed && (
            <div className="sidebar-version">
              <span className="online-dot" />
              All systems local <span>v0.3.0</span>
            </div>
          )}
        </div>
      </aside>

      <div className="app-main">
        <header className="topbar">
          <div className="topbar-leading">
            <button
              className="mobile-menu"
              aria-label="Open navigation"
              onClick={() => setMobileNav(true)}
            >
              â˜°
            </button>

            <div className="breadcrumb">
              <span>TurboFix</span>
              <span className="breadcrumb-slash">/</span>
              <strong>{pageTitle(page)}</strong>
            </div>
          </div>

          <div className="topbar-actions">
            <div className="environment-chip">
              <span className="online-dot" />
              Desktop app
            </div>
            <div className="topbar-divider" />
            <div className="user-chip">
              <span className="user-avatar">RM</span>
              <span className="user-name">Local user</span>
            </div>
          </div>
        </header>

        <main className="page-canvas">
          <div className="page-intro">
            <div>
              <div className="eyebrow">
                NETWORK HEALTH CENTER <span className="eyebrow-line" />
              </div>
              <h1 className="page-title">{pageTitle(page)}</h1>
              <p>{NAV.find((item) => item.id === page)?.hint}</p>
            </div>

            <div className="intro-date">
              <span className="intro-date-dot" />
              LOCAL MONITORING
            </div>
          </div>

          {loadError && (
            <div className="mb-4">
              <ErrorBanner
                message={loadError.message}
                onRetry={loadLatest}
              />
            </div>
          )}

          {speedHistoryError && (
            <div className="mb-4">
              <ErrorBanner
                message={`Speed-test history could not be loaded: ${speedHistoryError.message}`}
                onRetry={loadSpeedHistory}
              />
            </div>
          )}

          <div className="page-content" key={page}>
            {page === "overview" && (
              <Overview
                report={report}
                running={scan.running}
                cancelling={scan.cancelling}
                progress={scan.progress}
                scanError={scan.error}
                onScan={scan.start}
                onCancel={scan.cancel}
                perf={perf}
                onOpenDiagnostics={() => navigate("diagnostics")}
                speedTest={latestSpeedTest}
                onOpenSpeedTest={() => navigate("speedtest")}
              />
            )}

            {page === "diagnostics" && (
              <Diagnostics report={report} />
            )}

            {page === "speedtest" && (
              <SpeedTest
                api={api}
                latest={latestSpeedTest}
                history={speedHistory}
                onFinish={onSpeedTestFinished}
              />
            )}

            {page === "wifi" && <Wifi api={api} />}

            {page === "performance" && <Performance perf={perf} />}

            {page === "optimizer" && (
              <Optimizer api={api} onVerify={verifyDiagnostics} />
            )}

            {page === "history" && (
              <History
                api={api}
                version={version}
                onOpen={open}
                onChanged={reloadHistory}
                speedTests={speedHistory}
              />
            )}

            {page === "settings" && <Settings api={api} />}

            {page === "cleanup" && <Cleanup api={api} />}
          </div>

          <footer className="app-footer">
            <span>
              TurboFix <b>Â·</b> Diagnose. Fix. Verify.
            </span>
            <span>
              Measurements are estimates, not a promise of faster internet.
            </span>
          </footer>
        </main>
      </div>
    </div>
  );
}
