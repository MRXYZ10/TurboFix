//! SQLite persistence: scan history, performance samples and saved settings.
//! Schema changes go through `MIGRATIONS` (tracked in `PRAGMA user_version`).

use crate::config::ScanConfig;
use crate::diagnosis::OverallStatus;
use crate::error::{CoreError, Result};
use crate::performance::PerfSnapshot;
use crate::scan::{ScanReport, ScanStatus};
use crate::speedtest::SpeedTestResult;
use crate::stats::median;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

const MIGRATIONS: &[&str] = &[
    "
CREATE TABLE scans (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    started_at TEXT NOT NULL,
    duration_ms INTEGER NOT NULL,
    status TEXT NOT NULL,
    overall TEXT,
    finding_count INTEGER NOT NULL,
    median_latency_ms REAL,
    loss_pct REAL,
    median_dns_ms REAL,
    report_json TEXT NOT NULL
);
CREATE INDEX idx_scans_started ON scans(started_at DESC, id DESC);
CREATE TABLE perf_samples (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    ts TEXT NOT NULL,
    cpu_pct REAL NOT NULL,
    mem_used INTEGER NOT NULL,
    mem_total INTEGER NOT NULL
);
CREATE INDEX idx_perf_ts ON perf_samples(ts);
CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
",
    "
CREATE TABLE speed_tests (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    started_at TEXT NOT NULL,
    duration_ms INTEGER NOT NULL,
    download_mbps REAL NOT NULL,
    upload_mbps REAL NOT NULL,
    download_bytes INTEGER NOT NULL,
    upload_bytes INTEGER NOT NULL,
    download_duration_ms INTEGER NOT NULL,
    upload_duration_ms INTEGER NOT NULL,
    endpoint_label TEXT NOT NULL
);
CREATE INDEX idx_speed_tests_started ON speed_tests(started_at DESC, id DESC);
",
];

pub const SCHEMA_VERSION: u32 = MIGRATIONS.len() as u32;
const MAX_PERF_ROWS: u32 = 20_000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScanSummary {
    pub id: i64,
    pub started_at: String,
    pub duration_ms: u64,
    pub status: ScanStatus,
    pub overall: Option<OverallStatus>,
    pub finding_count: u32,
    pub median_latency_ms: Option<f64>,
    pub loss_pct: Option<f64>,
    pub median_dns_ms: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PerfPoint {
    pub ts: String,
    pub cpu_pct: f64,
    pub mem_used: u64,
    pub mem_total: u64,
}

pub struct Storage {
    conn: Mutex<Connection>,
}

impl Storage {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let _ = conn.query_row("PRAGMA journal_mode=WAL", [], |_| Ok(())); // best effort
        let s = Self {
            conn: Mutex::new(conn),
        };
        s.migrate()?;
        Ok(s)
    }

    fn lock(&self) -> Result<MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|_| CoreError::Internal("database lock poisoned".into()))
    }

    pub fn schema_version(&self) -> Result<u32> {
        Ok(self
            .lock()?
            .query_row("PRAGMA user_version", [], |r| r.get::<_, u32>(0))?)
    }

    fn migrate(&self) -> Result<()> {
        let mut conn = self.lock()?;
        let current: u32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if current > SCHEMA_VERSION {
            return Err(CoreError::Storage(format!(
                "database schema v{current} is newer than this app supports (v{SCHEMA_VERSION}); refusing to open it"
            )));
        }
        for (i, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
            let tx = conn.transaction()?;
            tx.execute_batch(sql)?;
            tx.pragma_update(None, "user_version", (i + 1) as u32)?;
            tx.commit()?;
        }
        Ok(())
    }

    // ---- scans ---------------------------------------------------------

    pub fn save_scan(&self, r: &ScanReport) -> Result<i64> {
        let overall = r
            .diagnosis
            .as_ref()
            .and_then(|d| serde_json::to_value(d.status).ok())
            .and_then(|v| v.as_str().map(str::to_owned));
        let findings = r.diagnosis.as_ref().map(|d| d.findings.len()).unwrap_or(0) as i64;
        let medians: Vec<f64> = r.latency.iter().filter_map(|l| l.stats.median_ms).collect();
        let (sent, received) = r.latency.iter().fold((0u32, 0u32), |(s, rc), l| {
            (s + l.stats.sent, rc + l.stats.received)
        });
        let loss = (sent > 0).then(|| crate::stats::loss_pct(sent, received));
        let dns: Vec<f64> = r.dns.iter().filter_map(|d| d.time_ms).collect();
        let mut stored = r.clone();
        stored.id = None;
        let json = serde_json::to_string(&stored)?;
        let conn = self.lock()?;
        conn.execute(
            "INSERT INTO scans (started_at, duration_ms, status, overall, finding_count, median_latency_ms, loss_pct, median_dns_ms, report_json)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![r.started_at, r.duration_ms as i64, r.status.as_str(), overall, findings, median(&medians), loss, median(&dns), json],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn get_scan(&self, id: i64) -> Result<ScanReport> {
        let json: Option<String> = self
            .lock()?
            .query_row("SELECT report_json FROM scans WHERE id=?1", [id], |r| {
                r.get(0)
            })
            .optional()?;
        let mut report: ScanReport =
            serde_json::from_str(&json.ok_or_else(|| CoreError::NotFound(format!("scan {id}")))?)?;
        report.id = Some(id);
        Ok(report)
    }

    pub fn list_scans(&self, limit: u32) -> Result<Vec<ScanSummary>> {
        let conn = self.lock()?;
        let mut st = conn.prepare(
            "SELECT id, started_at, duration_ms, status, overall, finding_count, median_latency_ms, loss_pct, median_dns_ms
             FROM scans ORDER BY started_at DESC, id DESC LIMIT ?1",
        )?;
        let rows = st.query_map([limit.min(1000)], |r| {
            let overall: Option<String> = r.get(4)?;
            Ok(ScanSummary {
                id: r.get(0)?,
                started_at: r.get(1)?,
                duration_ms: r.get::<_, i64>(2)? as u64,
                status: ScanStatus::parse(&r.get::<_, String>(3)?),
                overall: overall
                    .and_then(|s| serde_json::from_value(serde_json::Value::String(s)).ok()),
                finding_count: r.get::<_, i64>(5)? as u32,
                median_latency_ms: r.get(6)?,
                loss_pct: r.get(7)?,
                median_dns_ms: r.get(8)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn delete_scan(&self, id: i64) -> Result<()> {
        if self
            .lock()?
            .execute("DELETE FROM scans WHERE id=?1", [id])?
            == 0
        {
            return Err(CoreError::NotFound(format!("scan {id}")));
        }
        Ok(())
    }

    pub fn clear_history(&self) -> Result<u64> {
        let conn = self.lock()?;
        let scans = conn.execute("DELETE FROM scans", [])? as u64;
        let speeds = conn.execute("DELETE FROM speed_tests", [])? as u64;
        let perf = conn.execute("DELETE FROM perf_samples", [])? as u64;
        Ok(scans + speeds + perf)
    }

    // ---- speed-test history -------------------------------------------

    pub fn save_speed_test(&self, result: &SpeedTestResult) -> Result<SpeedTestResult> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT INTO speed_tests (started_at, duration_ms, download_mbps, upload_mbps, download_bytes, upload_bytes, download_duration_ms, upload_duration_ms, endpoint_label)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                result.started_at,
                result.duration_ms as i64,
                result.download_mbps,
                result.upload_mbps,
                result.download_bytes as i64,
                result.upload_bytes as i64,
                result.download_duration_ms as i64,
                result.upload_duration_ms as i64,
                result.endpoint_label,
            ],
        )?;
        let id = conn.last_insert_rowid();
        // Keep local history bounded even when a user repeats the test frequently.
        conn.execute("DELETE FROM speed_tests WHERE id NOT IN (SELECT id FROM speed_tests ORDER BY started_at DESC, id DESC LIMIT 100)", [])?;
        let mut saved = result.clone();
        saved.id = Some(id);
        Ok(saved)
    }

    pub fn list_speed_tests(&self, limit: u32) -> Result<Vec<SpeedTestResult>> {
        let conn = self.lock()?;
        let mut st = conn.prepare(
            "SELECT id, started_at, duration_ms, download_mbps, upload_mbps, download_bytes, upload_bytes, download_duration_ms, upload_duration_ms, endpoint_label
             FROM speed_tests ORDER BY started_at DESC, id DESC LIMIT ?1",
        )?;
        let rows = st.query_map([limit.min(100)], |r| {
            Ok(SpeedTestResult {
                id: Some(r.get(0)?),
                started_at: r.get(1)?,
                duration_ms: r.get::<_, i64>(2)? as u64,
                download_mbps: r.get(3)?,
                upload_mbps: r.get(4)?,
                download_bytes: r.get::<_, i64>(5)? as u64,
                upload_bytes: r.get::<_, i64>(6)? as u64,
                download_duration_ms: r.get::<_, i64>(7)? as u64,
                upload_duration_ms: r.get::<_, i64>(8)? as u64,
                endpoint_label: r.get(9)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    /// Retention: drop scans older than `older_than_days`, then keep only the newest `keep_latest`.
    pub fn prune_scans(&self, older_than_days: u32, keep_latest: u32) -> Result<u64> {
        let cutoff =
            (chrono::Utc::now() - chrono::Duration::days(older_than_days as i64)).to_rfc3339();
        let conn = self.lock()?;
        let a = conn.execute("DELETE FROM scans WHERE started_at < ?1", [cutoff])?;
        let b = conn.execute("DELETE FROM scans WHERE id NOT IN (SELECT id FROM scans ORDER BY started_at DESC, id DESC LIMIT ?1)", [keep_latest])?;
        Ok((a + b) as u64)
    }

    // ---- performance samples ------------------------------------------

    pub fn record_perf(&self, s: &PerfSnapshot) -> Result<()> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT INTO perf_samples (ts, cpu_pct, mem_used, mem_total) VALUES (?1,?2,?3,?4)",
            params![
                s.timestamp,
                s.cpu_pct as f64,
                s.memory_used_bytes as i64,
                s.memory_total_bytes as i64
            ],
        )?;
        // Bound growth even if the app runs for weeks.
        conn.execute(
            "DELETE FROM perf_samples WHERE id <= (SELECT MAX(id) FROM perf_samples) - ?1",
            [MAX_PERF_ROWS],
        )?;
        Ok(())
    }

    /// Newest `limit` samples, oldest first (ready for charting).
    pub fn recent_perf(&self, limit: u32) -> Result<Vec<PerfPoint>> {
        let conn = self.lock()?;
        let mut st = conn.prepare(
            "SELECT ts, cpu_pct, mem_used, mem_total FROM perf_samples ORDER BY id DESC LIMIT ?1",
        )?;
        let mut v = st
            .query_map([limit.min(10_000)], |r| {
                Ok(PerfPoint {
                    ts: r.get(0)?,
                    cpu_pct: r.get(1)?,
                    mem_used: r.get::<_, i64>(2)? as u64,
                    mem_total: r.get::<_, i64>(3)? as u64,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        v.reverse();
        Ok(v)
    }

    pub fn prune_perf(&self, older_than_days: u32) -> Result<u64> {
        let cutoff =
            (chrono::Utc::now() - chrono::Duration::days(older_than_days as i64)).to_rfc3339();
        Ok(self
            .lock()?
            .execute("DELETE FROM perf_samples WHERE ts < ?1", [cutoff])? as u64)
    }

    // ---- settings ------------------------------------------------------

    /// Stored config, or defaults if none/corrupt/invalid.
    pub fn load_config(&self) -> Result<ScanConfig> {
        let json: Option<String> = self
            .lock()?
            .query_row(
                "SELECT value FROM settings WHERE key='scan_config'",
                [],
                |r| r.get(0),
            )
            .optional()?;
        Ok(
            match json.and_then(|j| serde_json::from_str::<ScanConfig>(&j).ok()) {
                Some(c) if c.validate().is_ok() => c,
                _ => ScanConfig::default(),
            },
        )
    }

    pub fn save_config(&self, c: &ScanConfig) -> Result<()> {
        c.validate()?;
        self.lock()?.execute(
            "INSERT INTO settings (key, value) VALUES ('scan_config', ?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [serde_json::to_string(c)?],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corrupt_stored_config_falls_back_to_defaults() {
        let s = Storage::open_in_memory().unwrap();
        s.lock()
            .unwrap()
            .execute(
                "INSERT INTO settings (key,value) VALUES ('scan_config','{nope')",
                [],
            )
            .unwrap();
        assert_eq!(s.load_config().unwrap(), ScanConfig::default());
    }

    #[test]
    fn speed_test_history_persists_and_clear_history_removes_it() {
        let s = Storage::open_in_memory().unwrap();
        let sample = SpeedTestResult {
            id: None,
            started_at: chrono::Utc::now().to_rfc3339(),
            duration_ms: 2200,
            download_mbps: 85.25,
            upload_mbps: 12.5,
            download_bytes: 5_000_000,
            upload_bytes: 1_000_000,
            download_duration_ms: 410,
            upload_duration_ms: 600,
            endpoint_label: "Cloudflare speed-test edge".into(),
        };
        let saved = s.save_speed_test(&sample).unwrap();
        assert!(saved.id.is_some());
        let history = s.list_speed_tests(10).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].download_mbps, 85.25);
        assert_eq!(history[0].upload_mbps, 12.5);
        assert_eq!(s.clear_history().unwrap(), 1);
        assert!(s.list_speed_tests(10).unwrap().is_empty());
    }

    /// Regression: without serde_json's `float_roundtrip` feature, stored measurements
    /// came back one ULP different from what was measured.
    #[test]
    fn measurements_roundtrip_through_json_exactly() {
        for v in [
            0.226257_f64,
            0.105856,
            0.158511,
            0.20807799999999999,
            12.345678901234567,
        ] {
            let back: f64 = serde_json::from_str(&serde_json::to_string(&v).unwrap()).unwrap();
            assert_eq!(back, v);
        }
    }

    #[test]
    fn prune_scans_keeps_newest_and_respects_age() {
        use crate::scan::*;
        let s = Storage::open_in_memory().unwrap();
        let mk = |when: &str| ScanReport {
            id: None,
            app_version: "t".into(),
            started_at: when.into(),
            duration_ms: 1,
            status: ScanStatus::Cancelled,
            error: None,
            platform: crate::platform::platform_info(),
            config: ScanConfig::default(),
            interfaces: vec![],
            gateway: None,
            dns: vec![],
            https: vec![],
            latency: vec![],
            diagnosis: None,
            performance: None,
            unavailable: vec![],
        };
        let now = chrono::Utc::now();
        for days in [200, 100, 3, 2, 1] {
            s.save_scan(&mk(&(now - chrono::Duration::days(days)).to_rfc3339()))
                .unwrap();
        }
        assert_eq!(s.prune_scans(90, 2).unwrap(), 3); // 2 too old + 1 over the cap
        let left = s.list_scans(10).unwrap();
        assert_eq!(left.len(), 2);
        assert!(left[0].started_at > left[1].started_at);
    }
}
