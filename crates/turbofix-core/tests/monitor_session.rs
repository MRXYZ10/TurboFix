use std::net::SocketAddr;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use turbofix_core::monitor::{run_monitor_session, MonitorConfig};
use turbofix_core::storage::Storage;

#[tokio::test]
async fn local_listener_and_refused_port_are_reported_separately() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let listening_addr: SocketAddr = listener.local_addr().unwrap();
    let acceptor =
        tokio::spawn(async move { while let Ok((_stream, _)) = listener.accept().await {} });
    let refused = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let refused_addr = refused.local_addr().unwrap();
    drop(refused);

    let config = MonitorConfig {
        targets: vec![listening_addr.to_string(), refused_addr.to_string()],
        interval_ms: 500,
        timeout_ms: 1_000,
        max_concurrency: 2,
    };
    let cancel = CancellationToken::new();
    let stop = cancel.clone();
    let summary = run_monitor_session(config, cancel, move |snapshot| {
        if snapshot.total_attempts >= 2 {
            stop.cancel();
        }
    })
    .await
    .unwrap();
    acceptor.abort();

    assert_eq!(summary.total_attempts, 2);
    let connected = summary
        .targets
        .iter()
        .find(|t| t.target == listening_addr.to_string())
        .unwrap();
    assert_eq!(connected.successful_attempts, 1);
    assert!(connected.latest_success_latency_ms.is_some());
    let rejected = summary
        .targets
        .iter()
        .find(|t| t.target == refused_addr.to_string())
        .unwrap();
    assert_eq!(rejected.attempts, 1);
    assert_eq!(
        rejected.connection_refusals + rejected.connection_timeouts + rejected.transport_errors,
        1,
        "the deliberately closed loopback port must produce a classified failure"
    );
}

#[tokio::test]
async fn stopped_session_summary_survives_file_database_reopen() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let listening_addr = listener.local_addr().unwrap();
    let acceptor =
        tokio::spawn(async move { while let Ok((_stream, _)) = listener.accept().await {} });
    let refused = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let refused_addr = refused.local_addr().unwrap();
    drop(refused);

    let config = MonitorConfig {
        targets: vec![listening_addr.to_string(), refused_addr.to_string()],
        interval_ms: 500,
        timeout_ms: 1_000,
        max_concurrency: 2,
    };
    let cancel = CancellationToken::new();
    let stop = cancel.clone();
    let summary = run_monitor_session(config, cancel, move |snapshot| {
        if snapshot.total_attempts >= 2 {
            stop.cancel();
        }
    })
    .await
    .unwrap();
    acceptor.abort();

    let directory = tempfile::tempdir().unwrap();
    let database_path = directory.path().join("turbofix.sqlite3");
    let saved = {
        let storage = Storage::open(&database_path).unwrap();
        storage.save_monitor_session(&summary).unwrap()
    };
    let reopened = Storage::open(&database_path).unwrap();
    let history = reopened.list_monitor_sessions(10).unwrap();

    assert_eq!(history, vec![saved.clone()]);
    let persisted = &history[0];
    assert!(persisted.id.is_some());
    assert_eq!(persisted.started_at, summary.started_at);
    assert_eq!(persisted.ended_at, summary.ended_at);
    assert!(chrono::DateTime::parse_from_rfc3339(&persisted.started_at).is_ok());
    assert!(chrono::DateTime::parse_from_rfc3339(&persisted.ended_at).is_ok());
    assert!(persisted.duration_ms > 0);
    assert_eq!(persisted.interval_ms, 500);
    assert_eq!(persisted.timeout_ms, 1_000);
    assert_eq!(persisted.total_attempts, 2);
    assert_eq!(persisted.successful_attempts, 1);
    assert_eq!(persisted.failed_attempts, 1);

    let connected = persisted
        .targets
        .iter()
        .find(|target| target.target == listening_addr.to_string())
        .unwrap();
    assert_eq!(connected.attempts, 1);
    assert_eq!(connected.successful_attempts, 1);
    assert_eq!(connected.failed_attempts, 0);
    assert!(connected.latest_success_latency_ms.is_some());
    assert!(matches!(
        connected.last_result.as_ref().map(|result| &result.outcome),
        Some(turbofix_core::network::ProbeOutcome::Connected { .. })
    ));

    let rejected = persisted
        .targets
        .iter()
        .find(|target| target.target == refused_addr.to_string())
        .unwrap();
    assert_eq!(rejected.attempts, 1);
    assert_eq!(rejected.successful_attempts, 0);
    assert_eq!(rejected.failed_attempts, 1);
    assert_eq!(
        rejected.connection_refusals
            + rejected.resolution_failures
            + rejected.resolution_timeouts
            + rejected.connection_timeouts
            + rejected.transport_errors,
        1,
        "the failed loopback outcome category must survive persistence: {rejected:?}"
    );
    assert!(rejected.last_result.is_some());
    assert!(rejected.latest_success_latency_ms.is_none());
}
