
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
import PingMonitor from "./pages/PingMonitor";
import Wifi from "./pages/Wifi";
import Optimizer from "./pages/Optimizer";
import Settings from "./pages/Settings";

import type {
  ApiError,
  Capability,
  ScanReport,
  SpeedTestResult,
} from "./types";
import { ErrorBanner } from "./components/ui";

type PageId =
  | "overview"
  | "diagnostics"
  | "wifi"
  | "speedtest"
  | "ping"
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
    icon: "OV",
    section: "Workspace",
    hint: "Your network at a glance",
  },
  {
    id: "diagnostics",
    label: "Diagnostics",
    icon: "DX",
    section: "Workspace",
    hint: "Find the root cause",
  },
  {
    id: "speedtest",
    label: "Speed test",
    icon: "ST",
    section: "Workspace",
    hint: "Measure real throughput",
  },
  {
    id: "ping",
    label: "Ping monitor",
    icon: "DX",
    section: "Workspace",
    hint: "Monitor TCP response time",
  },
  {
    id: "wifi",
    label: "Wi-Fi analysis",
    icon: "WF",
    section: "Tools",
    hint: "Inspect your wireless link",
  },
  {
    id: "performance",
    label: "PC performance",
    icon: "PC",
    section: "Tools",
    hint: "Live system health",
  },
  {
    id: "optimizer",
    label: "Smart optimizer",
    icon: "FX",
    section: "Tools",
    hint: "Safe, evidence-based fixes",
  },
  {
    id: "cleanup",
    label: "Storage cleanup",
    icon: "CL",
    section: "Tools",
    hint: "Review temporary files",
  },
  {
    id: "history",
    label: "Reports & history",
    icon: "HI",
    section: "Manage",
    hint: "Your local measurements",
  },
  {
    id: "settings",
    label: "Settings",
    icon: "SE",
    section: "Manage",
    hint: "Configure TurboFix",
  },
];

const isAndroidWebView = () => typeof navigator !== "undefined" && /Android/i.test(navigator.userAgent);
const pageTitle = (page: PageId) =>
  page === "wifi" && isAndroidWebView() ? "Mobile Boost" : NAV.find((item) => item.id === page)?.label ?? "Overview";

function NavGlyph({ name }: { name: PageId }) {
  return (
    <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" focusable="false">
      {name === "overview" && <>
        <rect x="3.5" y="3.5" width="7" height="7" rx="1.5" />
        <rect x="13.5" y="3.5" width="7" height="7" rx="1.5" />
        <rect x="3.5" y="13.5" width="7" height="7" rx="1.5" />
        <rect x="13.5" y="13.5" width="7" height="7" rx="1.5" />
      </>}
      {name === "diagnostics" && <path d="M3 12h4l2.2-5.5L13 18l2.2-5H21" />}
      {name === "wifi" && <>
        <path d="M3 9.5a14 14 0 0 1 18 0" />
        <path d="M6.5 13a8.5 8.5 0 0 1 11 0" />
        <path d="M9.8 16.3a3.5 3.5 0 0 1 4.4 0" />
        <circle cx="12" cy="19.5" r=".8" fill="currentColor" stroke="none" />
      </>}
      {name === "speedtest" && <>
        <path d="M5 19a9 9 0 1 1 14 0" />
        <path d="m12 12 4-4" />
        <circle cx="12" cy="12" r="1.4" />
      </>}
      {name === "performance" && <path d="M3 12h4l2-6 4 12 2-6h6" />}
      {name === "optimizer" && <>
        <path d="m12 3 1.5 5.5L19 10l-5.5 1.5L12 17l-1.5-5.5L5 10l5.5-1.5L12 3Z" />
        <path d="m19 15 .8 2.2L22 18l-2.2.8L19 21l-.8-2.2L16 18l2.2-.8L19 15Z" />
      </>}
      {name === "cleanup" && <>
        <path d="M4 7h16" />
        <path d="m6 7 1 13h10l1-13" />
        <path d="M9 7V4h6v3M10 10v7M14 10v7" />
      </>}
      {name === "history" && <>
        <circle cx="12" cy="12" r="8.5" />
        <path d="M12 7v5l3.5 2" />
      </>}
      {name === "settings" && <>
        <circle cx="12" cy="12" r="3" />
        <path d="m19.4 15 .1.1 1.1.9-1.5 2.6-1.4-.6a8 8 0 0 1-1.5.9l-.2 1.5h-3l-.3-1.5a8 8 0 0 1-1.5-.9l-1.4.6-1.5-2.6 1.2-.9a7 7 0 0 1 0-1.8l-1.2-.9 1.5-2.6 1.4.6a8 8 0 0 1 1.5-.9l.3-1.5h3l.2 1.5a8 8 0 0 1 1.5.9l1.4-.6 1.5 2.6-1.1.9a7 7 0 0 1 0 1.7Z" transform="translate(-1 -1) scale(1.05)" />
      </>}
    </svg>
  );
}
export default function App({ api = realApi }: { api?: Api }) {
  const [page, setPage] = useState<PageId>("overview");
  const [collapsed, setCollapsed] = useState(false);
  const [mobileNav, setMobileNav] = useState(false);
  const [capabilities, setCapabilities] = useState<Capability[] | null>(null);
  const isAndroid = typeof navigator !== "undefined" && /Android/i.test(navigator.userAgent);

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
    let active = true;
    void api.capabilities().then((items) => {
      if (active) setCapabilities(items);
    }).catch(() => {
      // Keep conservative Android navigation if capability lookup fails.
      if (active) setCapabilities(null);
    });
    return () => {
      active = false;
    };
  }, [api]);

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

  // Accept the optimizer's completed verification scan into the shared report.
  const verifyDiagnostics = (verified: ScanReport) => {
    onFinished(verified);
  };

  const groups = ["Workspace", "Tools", "Manage"] as const;
  const capabilityIdByPage: Partial<Record<PageId, string>> = {
    diagnostics: "connectivity",
    wifi: "wifi",
    speedtest: "speed_test",
    performance: "pc_monitor",
    optimizer: "optimizer",
    cleanup: "cleanup",
    history: "history",
  };
  const blockedAndroidFallback = new Set<PageId>(["wifi", "optimizer", "cleanup"]);
  const visibleNavItems = !isAndroid ? NAV : NAV.filter((item) => {
    if (!capabilities) return !blockedAndroidFallback.has(item.id);
    const capabilityId = capabilityIdByPage[item.id];
    if (!capabilityId) return true;
    return capabilities.some((capability) =>
      capability.id === capabilityId && capability.status === "available",
    );
  });
  const displayPageTitle = isAndroid && page === "performance"
    ? "Device health"
    : pageTitle(page);

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
            {collapsed ? ">>" : "<<"}
          </button>
        </div>

        <div className="workspace-pill">
          <span className="workspace-avatar">R</span>
          {!collapsed && (
            <>
              <span className="workspace-label">Personal workspace</span>
              <span className="workspace-chevron">v</span>
            </>
          )}
        </div>

        <nav aria-label="Main navigation" className="main-nav">
          {groups.map((group) => (
            <div className="nav-group" key={group}>
              {!collapsed && (
                <div className="nav-section-label">{group}</div>
              )}

              {visibleNavItems.filter((item) => item.section === group).map((item) => (
                <button
                  key={item.id}
                  onClick={() => navigate(item.id)}
                  aria-current={page === item.id ? "page" : undefined}
                  title={collapsed ? item.label : undefined}
                  className={`nav-item ${
                    page === item.id ? "nav-item-active" : ""
                  }`}
                >
                  <span className="nav-icon" aria-hidden="true"><NavGlyph name={item.id} /></span>

                  {!collapsed && (
                    <>
                      <span className="nav-label">{isAndroid && item.id === "performance" ? "Device health" : item.label}</span>
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
            <span className="privacy-icon">OK</span>
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
            ><span className="menu-glyph" aria-hidden="true" /></button>

            <div className="breadcrumb">
              <span>TurboFix</span>
              <span className="breadcrumb-slash">/</span>
              <strong>{displayPageTitle}</strong>
            </div>
          </div>

          <div className="topbar-actions">
            <div className="environment-chip">
              <span className="online-dot" />
              {isAndroid ? "Android app" : "Desktop app"}
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
              <h1 className="page-title">{displayPageTitle}</h1>
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
            {page === "ping" && <PingMonitor api={api} />}

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
              TurboFix <b>·</b> Diagnose. Fix. Verify.
            </span>
            <span>
              Measurements are estimates, not a promise of faster internet.
            </span>
          </footer>
        </main>
      <nav className="mobile-bottom-nav" aria-label="Mobile navigation">
        {([
          { id: "overview", label: "Home" },
          { id: "diagnostics", label: "Scan" },
          { id: "speedtest", label: "Speed" },
          { id: "wifi", label: isAndroidWebView() ? "Boost" : "Wi-Fi" },
          { id: "performance", label: "Health" },
        ] as { id: PageId; label: string }[]).filter((tab) => !isAndroid || tab.id === "wifi" || visibleNavItems.some((item) => item.id === tab.id)).map((tab) => (
          <button
            key={tab.id}
            type="button"
            className="mobile-tab"
            aria-current={page === tab.id ? "page" : undefined}
            onClick={() => navigate(tab.id)}
          >
            <span className="mobile-tab-icon" aria-hidden="true"><NavGlyph name={tab.id} /></span>
            <span>{tab.label}</span>
          </button>
        ))}
        <button type="button" className="mobile-tab" onClick={() => setMobileNav(true)} aria-label="More tools and settings">
          <span className="mobile-tab-icon" aria-hidden="true">
            <svg viewBox="0 0 24 24" width="20" height="20" fill="currentColor" focusable="false">
              <circle cx="5" cy="12" r="1.7" /><circle cx="12" cy="12" r="1.7" /><circle cx="19" cy="12" r="1.7" />
            </svg>
          </span>
          <span>More</span>
        </button>
      </nav>
      </div>
    </div>
  );
}
