//! Integration tests: persistence, migrations, failed/cancelled/real scans.
//! They use only loopback networking, so no internet is required.

use std::time::Duration;
use tokio_util::sync::CancellationToken;
use turbofix_core::config::ScanConfig;
use turbofix_core::network::{tcp_connect, ProbeOutcome};
use turbofix_core::performance::PerfMonitor;
use turbofix_core::scan::{run_scan, ScanStatus};
use turbofix_core::storage::Storage;

fn local_config(port: u16) -> ScanConfig {
    ScanConfig {
        dns_hosts: vec!["localhost".into()],
        https_endpoints: vec![],
        latency_targets: vec![format!("127.0.0.1:{port}")],
        probe_count: 3,
        probe_interval_ms: 50,
        probe_timeout_ms: 500,
        dns_timeout_ms: 2000,
        ..ScanConfig::default()
    }
}

#[tokio::test]
async fn scan_runs_and_persists_across_reopen() {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.db");

    let report = run_scan(local_config(port), CancellationToken::new(), |_| {})
        .await
        .unwrap();
    assert_eq!(report.status, ScanStatus::Completed);
    assert_eq!(report.latency[0].stats.received, 3);
    assert!(report.dns.iter().all(|d| d.ok));
    assert!(report.diagnosis.is_some());
    assert!(report.unavailable.iter().any(|u| u.name.contains("speed")));

    let id = {
        let s = Storage::open(&path).unwrap();
        assert_eq!(
            s.schema_version().unwrap(),
            turbofix_core::storage::SCHEMA_VERSION
        );
        s.save_scan(&report).unwrap()
    };
    // "restart": new Storage on the same file
    let s = Storage::open(&path).unwrap();
    let list = s.list_scans(10).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, id);
    assert_eq!(list[0].status, ScanStatus::Completed);
    assert!(list[0].median_latency_ms.is_some());
    assert_eq!(list[0].loss_pct, None);
    let back = s.get_scan(id).unwrap();
    assert_eq!(back.id, Some(id));
    assert_eq!(back.latency, report.latency);
    drop(l);
}

#[tokio::test]
async fn cancelled_scan_is_reported_and_persisted_as_cancelled() {
    let tok = CancellationToken::new();
    let t2 = tok.clone();
    let cfg = ScanConfig {
        probe_count: 50,
        probe_interval_ms: 500,
        ..local_config(9)
    };
    let h = tokio::spawn(async move { run_scan(cfg, tok, |_| {}).await });
    tokio::time::sleep(Duration::from_millis(300)).await;
    t2.cancel();
    let report = tokio::time::timeout(Duration::from_secs(5), h)
        .await
        .expect("scan must stop promptly")
        .unwrap()
        .unwrap();
    assert_eq!(report.status, ScanStatus::Cancelled);
    assert!(report.diagnosis.is_none(), "no diagnosis from partial data");
    let s = Storage::open_in_memory().unwrap();
    let id = s.save_scan(&report).unwrap();
    assert_eq!(s.get_scan(id).unwrap().status, ScanStatus::Cancelled);
}

#[tokio::test]
async fn invalid_config_is_rejected_before_any_probe() {
    let cfg = ScanConfig {
        probe_count: 0,
        ..ScanConfig::default()
    };
    let err = run_scan(cfg, CancellationToken::new(), |_| {})
        .await
        .unwrap_err();
    assert_eq!(turbofix_core::ApiError::from(err).code, "invalid_config");
}

#[tokio::test]
async fn unreachable_targets_never_produce_fake_latency() {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    drop(l);
    let cfg = ScanConfig {
        dns_hosts: vec!["does-not-exist.invalid".into()],
        latency_targets: vec!["does-not-exist.invalid:443".into()],
        ..local_config(port)
    };
    let report = run_scan(cfg, CancellationToken::new(), |_| {})
        .await
        .unwrap();
    assert_eq!(report.latency[0].stats.median_ms, None);
    assert_eq!(report.latency[0].stats.loss_pct, None);
    assert_eq!(
        report.latency[0].stats.resolution_failed + report.latency[0].stats.resolution_timeouts,
        3
    );
    assert!(report.dns.iter().all(|d| d.time_ms.is_none()));
}

#[tokio::test]
async fn closed_loopback_port_is_distinguished_from_timeout() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let outcome = tcp_connect(
        &addr.to_string(),
        Duration::from_secs(10),
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert!(
        !matches!(outcome, ProbeOutcome::Connected { .. }),
        "closed port cannot be a successful connect: {outcome:?}"
    );
    assert!(
        matches!(
            outcome,
            ProbeOutcome::ConnectionRefused { .. }
                | ProbeOutcome::Timeout { .. }
                | ProbeOutcome::TransportError { .. }
        ),
        "unexpected outcome: {outcome:?}"
    );
}

#[test]
fn migrations_are_idempotent_and_newer_schema_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("m.db");
    drop(Storage::open(&path).unwrap());
    assert_eq!(
        Storage::open(&path).unwrap().schema_version().unwrap(),
        turbofix_core::storage::SCHEMA_VERSION
    );
    {
        let c = rusqlite::Connection::open(&path).unwrap();
        c.pragma_update(None, "user_version", 99).unwrap();
    }
    assert!(Storage::open(&path).is_err());
}

#[test]
fn history_crud_retention_and_clear() {
    let s = Storage::open_in_memory().unwrap();
    let base = sample_report();
    let a = s.save_scan(&base).unwrap();
    let b = s.save_scan(&base).unwrap();
    assert!(b > a);
    assert_eq!(s.list_scans(10).unwrap().len(), 2);
    s.delete_scan(a).unwrap();
    assert!(matches!(
        s.get_scan(a),
        Err(turbofix_core::CoreError::NotFound(_))
    ));
    assert!(matches!(
        s.delete_scan(a),
        Err(turbofix_core::CoreError::NotFound(_))
    ));
    s.clear_history().unwrap();
    assert!(s.list_scans(10).unwrap().is_empty());
}

#[test]
fn perf_samples_persist_in_order() {
    let s = Storage::open_in_memory().unwrap();
    let mut m = PerfMonitor::new();
    for _ in 0..3 {
        s.record_perf(&m.sample()).unwrap();
    }
    let pts = s.recent_perf(10).unwrap();
    assert_eq!(pts.len(), 3);
    assert!(pts.windows(2).all(|w| w[0].ts <= w[1].ts));
    assert!(pts[0].mem_total > 0);
}

#[test]
fn config_roundtrip_and_invalid_not_saved() {
    let s = Storage::open_in_memory().unwrap();
    assert_eq!(s.load_config().unwrap(), ScanConfig::default());
    let mut c = ScanConfig {
        probe_count: 7,
        ..ScanConfig::default()
    };
    s.save_config(&c).unwrap();
    assert_eq!(s.load_config().unwrap().probe_count, 7);
    c.probe_count = 0;
    assert!(s.save_config(&c).is_err());
    assert_eq!(s.load_config().unwrap().probe_count, 7);
}

fn sample_report() -> turbofix_core::scan::ScanReport {
    use turbofix_core::scan::*;
    ScanReport {
        id: None,
        app_version: APP_VERSION.into(),
        started_at: chrono_now(),
        duration_ms: 10,
        status: ScanStatus::Failed,
        error: Some("test".into()),
        platform: turbofix_core::platform::platform_info(),
        config: ScanConfig::default(),
        interfaces: vec![],
        gateway: None,
        dns: vec![],
        https: vec![],
        latency: vec![],
        diagnosis: None,
        performance: None,
        unavailable: unavailable_metrics(),
    }
}

fn chrono_now() -> String {
    "2026-01-01T00:00:00Z".to_string()
}
