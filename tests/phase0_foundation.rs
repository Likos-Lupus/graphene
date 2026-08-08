use graphene::{
    Artifact, ArtifactIntegrity, ArtifactSource, CachePolicy, DownloadDisposition, ErrorCode,
    Graphene, NetworkConfig, OperationEventKind, OperationResult, OperationState, Progress,
    ProxyPolicy, RetryPolicy,
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use tempfile::TempDir;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};

const FIXTURE_SHA1: &str = "1ffcdf6a62830b76bbedf597069d306d781b6f74";
const FIXTURE_SHA256: &str = "0fa051629d6f04851721b0a8d6002f23ed322719d0e81b9df49991d2cf7bf828";
const SLOW_SHA1: &str = "f57daca5f8f0deb3df3f8078abc7a25eaef3ab84";
const SLOW_SHA256: &str = "7798722fa4cee54d96b16c9a6eee3c19dac3683e4a51bf53638df66a5d0d866f";

fn fixture_bytes() -> Vec<u8> {
    b"graphene-phase0-fixture-".repeat(4096)
}

fn slow_bytes() -> Vec<u8> {
    b"cancel-me-".repeat(65_536)
}

fn local_network() -> NetworkConfig {
    NetworkConfig {
        proxy: ProxyPolicy::None,
        ..NetworkConfig::default()
    }
}

struct FixtureServer {
    address: std::net::SocketAddr,
    counts: Arc<Mutex<HashMap<String, usize>>>,
    task: JoinHandle<()>,
}

impl FixtureServer {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind fixture server");
        let address = listener.local_addr().expect("fixture address");
        let counts = Arc::new(Mutex::new(HashMap::new()));
        let task_counts = Arc::clone(&counts);
        let task = tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let counts = Arc::clone(&task_counts);
                tokio::spawn(async move {
                    let _ = serve_connection(stream, counts).await;
                });
            }
        });
        Self {
            address,
            counts,
            task,
        }
    }

    fn url(&self, path: &str) -> String {
        format!("http://{}{}", self.address, path)
    }

    fn requests(&self, path: &str) -> usize {
        *self
            .counts
            .lock()
            .expect("request counts")
            .get(path)
            .unwrap_or(&0)
    }
}

impl Drop for FixtureServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn serve_connection(
    mut stream: TcpStream,
    counts: Arc<Mutex<HashMap<String, usize>>>,
) -> std::io::Result<()> {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 1024];
    loop {
        let read = stream.read(&mut buffer).await?;
        if read == 0 {
            return Ok(());
        }
        request.extend_from_slice(&buffer[..read]);
        if request.windows(4).any(|window| window == b"\r\n\r\n") || request.len() > 16 * 1024 {
            break;
        }
    }
    let request_line = String::from_utf8_lossy(&request);
    let path = request_line
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/")
        .to_owned();
    let request_number = {
        let mut counts = counts.lock().expect("request counts");
        let entry = counts.entry(path.clone()).or_default();
        *entry += 1;
        *entry
    };

    match path.as_str() {
        "/artifact" => send_body(&mut stream, &fixture_bytes(), Duration::ZERO).await?,
        "/slow" => send_body(&mut stream, &slow_bytes(), Duration::from_millis(3)).await?,
        "/retry" if request_number == 1 => {
            send_status(&mut stream, 503, "Service Unavailable").await?
        }
        "/retry" => send_body(&mut stream, &fixture_bytes(), Duration::ZERO).await?,
        "/always-retry" => send_status(&mut stream, 503, "Service Unavailable").await?,
        "/interrupted" if request_number == 1 => {
            send_truncated_body(&mut stream, &fixture_bytes()).await?
        }
        "/interrupted" => send_body(&mut stream, &fixture_bytes(), Duration::ZERO).await?,
        path if path.starts_with("/permanent") => {
            send_status(&mut stream, 404, "Not Found").await?
        }
        _ => send_status(&mut stream, 404, "Not Found").await?,
    }
    Ok(())
}

async fn send_status(stream: &mut TcpStream, code: u16, reason: &str) -> std::io::Result<()> {
    let response =
        format!("HTTP/1.1 {code} {reason}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
    stream.write_all(response.as_bytes()).await
}

async fn send_truncated_body(stream: &mut TcpStream, body: &[u8]) -> std::io::Result<()> {
    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes()).await?;
    stream.write_all(&body[..body.len() / 2]).await?;
    stream.flush().await?;
    stream.shutdown().await
}

async fn send_body(
    stream: &mut TcpStream,
    body: &[u8],
    chunk_delay: Duration,
) -> std::io::Result<()> {
    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes()).await?;
    for chunk in body.chunks(4096) {
        stream.write_all(chunk).await?;
        stream.flush().await?;
        if !chunk_delay.is_zero() {
            tokio::time::sleep(chunk_delay).await;
        } else {
            tokio::task::yield_now().await;
        }
    }
    Ok(())
}

fn artifact(url: String) -> Artifact {
    Artifact::new(
        vec![ArtifactSource::new(url)],
        ArtifactIntegrity::none()
            .with_sha1(FIXTURE_SHA1.parse().expect("fixture sha1"))
            .with_sha256(FIXTURE_SHA256.parse().expect("fixture sha256")),
    )
    .with_expected_size(fixture_bytes().len() as u64)
}

fn slow_artifact(url: String) -> Artifact {
    Artifact::new(
        vec![ArtifactSource::new(url)],
        ArtifactIntegrity::none()
            .with_sha1(SLOW_SHA1.parse().expect("slow sha1"))
            .with_sha256(SLOW_SHA256.parse().expect("slow sha256")),
    )
    .with_expected_size(slow_bytes().len() as u64)
}

fn cache_path(root: &std::path::Path, sha256: &str) -> std::path::PathBuf {
    root.join("cache")
        .join("objects")
        .join("sha256")
        .join(&sha256[..2])
        .join(sha256)
}

fn sha1_cache_path(root: &std::path::Path, sha1: &str) -> std::path::PathBuf {
    root.join("cache")
        .join("objects")
        .join("sha1")
        .join(&sha1[..2])
        .join(sha1)
}

fn temporary_directory_is_empty(root: &std::path::Path) -> bool {
    std::fs::read_dir(root.join("cache/downloads/temporary"))
        .expect("temporary directory")
        .next()
        .is_none()
}

async fn wait_for_requests(server: &FixtureServer, path: &str, expected: usize) {
    for _ in 0..100 {
        if server.requests(path) >= expected {
            return;
        }
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    panic!("fixture server did not receive {expected} request(s) for {path}");
}

#[tokio::test]
async fn phase0_acceptance_download_verify_commit_and_cache_reuse() {
    let server = FixtureServer::start().await;
    let root = TempDir::new().expect("temp root");
    let graphene = Graphene::builder(root.path())
        .network(local_network())
        .build()
        .await
        .expect("build Graphene");

    assert!(
        graphene
            .data_root()
            .join("cache/downloads/temporary")
            .is_dir()
    );
    assert!(graphene.data_root().join("cache/objects").is_dir());
    assert!(graphene.data_root().join(".graphene-layout.json").is_file());

    let parent = graphene.operations().create("acceptance-parent");
    parent.start().expect("parent starts");
    let prepared = graphene
        .artifacts()
        .acquire(artifact(server.url("/artifact")), Some(&parent.handle()));
    let operation = prepared.operation();
    let events = operation.subscribe();
    assert_eq!(operation.parent_id(), Some(parent.handle().id()));

    let result = prepared.await_result().await.expect("download succeeds");
    assert_eq!(result.disposition, DownloadDisposition::Downloaded);
    assert_eq!(result.bytes, fixture_bytes().len() as u64);
    assert_eq!(
        std::fs::read(&result.path).expect("cached bytes"),
        fixture_bytes()
    );
    assert_eq!(operation.current_state(), OperationState::Succeeded);
    assert_eq!(operation.await_result().await, OperationResult::Succeeded);
    assert!(temporary_directory_is_empty(graphene.data_root()));

    let mut saw_full_progress = false;
    let mut terminal_count = 0;
    while let Some(event) = events.try_next() {
        match event.kind {
            OperationEventKind::Progress {
                progress: Progress::Bytes { completed, .. },
            } if completed == fixture_bytes().len() as u64 => saw_full_progress = true,
            OperationEventKind::Completed
            | OperationEventKind::Failed { .. }
            | OperationEventKind::Cancelled => terminal_count += 1,
            _ => {}
        }
    }
    assert!(saw_full_progress);
    assert_eq!(terminal_count, 1);

    let requests_after_first = server.requests("/artifact");
    let repeated = graphene
        .artifacts()
        .acquire(artifact(server.url("/artifact")), None)
        .await_result()
        .await
        .expect("cache hit succeeds");
    assert_eq!(repeated.disposition, DownloadDisposition::CacheHit);
    assert_eq!(server.requests("/artifact"), requests_after_first);

    assert_eq!(
        parent.succeed().expect("parent succeeds"),
        OperationResult::Succeeded
    );
}

#[tokio::test]
async fn cancellation_during_stream_never_commits_partial_or_succeeds() {
    let server = FixtureServer::start().await;
    let root = TempDir::new().expect("temp root");
    let graphene = Graphene::builder(root.path())
        .network(local_network())
        .build()
        .await
        .expect("build Graphene");
    let prepared = graphene
        .artifacts()
        .acquire(slow_artifact(server.url("/slow")), None);
    let operation = prepared.operation();
    let events = operation.subscribe();
    let task = tokio::spawn(async move { prepared.await_result().await });

    tokio::time::sleep(Duration::from_millis(30)).await;
    operation.cancel();
    let error = task
        .await
        .expect("task joins")
        .expect_err("download must cancel");
    assert!(error.is_cancelled());
    assert_eq!(operation.current_state(), OperationState::Cancelled);
    assert!(!cache_path(graphene.data_root(), SLOW_SHA256).exists());
    assert!(temporary_directory_is_empty(graphene.data_root()));

    let mut completed = 0;
    let mut cancelled = 0;
    while let Some(event) = events.try_next() {
        match event.kind {
            OperationEventKind::Completed => completed += 1,
            OperationEventKind::Cancelled => cancelled += 1,
            _ => {}
        }
    }
    assert_eq!(completed, 0);
    assert_eq!(cancelled, 1);
}

#[tokio::test]
async fn cancellation_before_request_performs_no_network_io() {
    let server = FixtureServer::start().await;
    let root = TempDir::new().expect("temp root");
    let graphene = Graphene::builder(root.path())
        .network(local_network())
        .build()
        .await
        .expect("build Graphene");
    let prepared = graphene
        .artifacts()
        .acquire(artifact(server.url("/artifact")), None);
    let operation = prepared.operation();
    operation.cancel();
    let error = prepared
        .await_result()
        .await
        .expect_err("cancelled before start");
    assert!(error.is_cancelled());
    assert_eq!(operation.current_state(), OperationState::Cancelled);
    assert_eq!(server.requests("/artifact"), 0);
}

#[tokio::test]
async fn cancellation_in_pre_commit_stage_wins_before_atomic_commit() {
    let server = FixtureServer::start().await;
    let root = TempDir::new().expect("temp root");
    let graphene = Graphene::builder(root.path())
        .network(local_network())
        .build()
        .await
        .expect("build Graphene");
    let prepared = graphene
        .artifacts()
        .acquire(artifact(server.url("/artifact")), None);
    let operation = prepared.operation();
    let events = operation.subscribe();
    let task = tokio::spawn(async move { prepared.await_result().await });

    while let Some(event) = events.next().await {
        if matches!(
            event.kind,
            OperationEventKind::StageChanged { ref stage } if stage == "pre-commit"
        ) {
            operation.cancel();
            break;
        }
    }
    let error = task
        .await
        .expect("task joins")
        .expect_err("pre-commit cancellation must win");
    assert!(error.is_cancelled());
    assert_eq!(operation.current_state(), OperationState::Cancelled);
    assert!(!cache_path(graphene.data_root(), FIXTURE_SHA256).exists());
    assert!(temporary_directory_is_empty(graphene.data_root()));
}

#[tokio::test]
async fn hash_and_size_mismatch_never_become_cache_hits() {
    let server = FixtureServer::start().await;
    let root = TempDir::new().expect("temp root");
    let graphene = Graphene::builder(root.path())
        .network(local_network())
        .build()
        .await
        .expect("build Graphene");

    let wrong_sha256 = "1fa051629d6f04851721b0a8d6002f23ed322719d0e81b9df49991d2cf7bf828";
    let wrong_hash = Artifact::new(
        vec![ArtifactSource::new(server.url("/artifact"))],
        ArtifactIntegrity::none().with_sha256(wrong_sha256.parse().expect("wrong sha256")),
    )
    .with_expected_size(fixture_bytes().len() as u64);
    let wrong_hash = graphene.artifacts().acquire(wrong_hash, None);
    let hash_operation = wrong_hash.operation();
    let hash_events = hash_operation.subscribe();
    let error = wrong_hash.await_result().await.expect_err("hash mismatch");
    assert_eq!(error.code, ErrorCode::HashMismatch);
    assert_eq!(hash_operation.current_state(), OperationState::Failed);
    assert!(!cache_path(graphene.data_root(), wrong_sha256).exists());
    assert!(temporary_directory_is_empty(graphene.data_root()));
    assert!(
        !std::iter::from_fn(|| hash_events.try_next())
            .any(|event| matches!(event.kind, OperationEventKind::Completed))
    );

    let wrong_size = Artifact::new(
        vec![ArtifactSource::new(server.url("/artifact"))],
        ArtifactIntegrity::none().with_sha256(FIXTURE_SHA256.parse().expect("sha256")),
    )
    .with_expected_size(fixture_bytes().len() as u64 + 1);
    let wrong_size = graphene.artifacts().acquire(wrong_size, None);
    let size_operation = wrong_size.operation();
    let error = wrong_size.await_result().await.expect_err("size mismatch");
    assert_eq!(error.code, ErrorCode::DownloadSizeMismatch);
    assert_eq!(size_operation.current_state(), OperationState::Failed);
    assert!(!cache_path(graphene.data_root(), FIXTURE_SHA256).exists());
    assert!(temporary_directory_is_empty(graphene.data_root()));
}

#[tokio::test]
async fn transient_retry_and_source_fallback_are_bounded_and_provider_neutral() {
    let server = FixtureServer::start().await;
    let root = TempDir::new().expect("temp root");
    let mut network = local_network();
    network.retry_policy = RetryPolicy {
        max_attempts: 2,
        base_delay: Duration::from_millis(1),
        max_delay: Duration::from_millis(2),
    };
    let graphene = Graphene::builder(root.path())
        .network(network)
        .build()
        .await
        .expect("build Graphene");

    let retry = artifact(server.url("/retry"));
    let result = graphene
        .artifacts()
        .acquire(retry, None)
        .await_result()
        .await
        .expect("retry succeeds");
    assert_eq!(result.disposition, DownloadDisposition::Downloaded);
    assert_eq!(server.requests("/retry"), 2);

    std::fs::remove_file(&result.path).expect("remove cache to exercise fallback");
    let fallback = Artifact::new(
        vec![
            ArtifactSource::new(server.url("/permanent")).with_priority(0),
            ArtifactSource::new(server.url("/artifact")).with_priority(1),
        ],
        ArtifactIntegrity::none()
            .with_sha1(FIXTURE_SHA1.parse().expect("sha1"))
            .with_sha256(FIXTURE_SHA256.parse().expect("sha256")),
    )
    .with_expected_size(fixture_bytes().len() as u64);
    let result = graphene
        .artifacts()
        .acquire(fallback, None)
        .await_result()
        .await
        .expect("fallback succeeds");
    assert_eq!(result.disposition, DownloadDisposition::Downloaded);
    assert_eq!(server.requests("/permanent"), 1);
    assert_eq!(server.requests("/artifact"), 1);
}

#[tokio::test]
async fn duplicate_requests_share_one_committed_object_without_corruption() {
    let server = FixtureServer::start().await;
    let root = TempDir::new().expect("temp root");
    let graphene = Graphene::builder(root.path())
        .network(local_network())
        .build()
        .await
        .expect("build Graphene");
    let first = graphene
        .artifacts()
        .acquire(artifact(server.url("/artifact")), None);
    let second = graphene
        .artifacts()
        .acquire(artifact(server.url("/artifact")), None);
    let (first, second) = tokio::join!(first.await_result(), second.await_result());
    let first = first.expect("first succeeds");
    let second = second.expect("second succeeds");
    assert_eq!(first.path, second.path);
    assert_eq!(
        std::fs::read(&first.path).expect("cache bytes"),
        fixture_bytes()
    );
    assert_eq!(server.requests("/artifact"), 1);
    assert!(matches!(
        (first.disposition, second.disposition),
        (
            DownloadDisposition::Downloaded,
            DownloadDisposition::CacheHit
        ) | (
            DownloadDisposition::CacheHit,
            DownloadDisposition::Downloaded
        )
    ));
}

#[tokio::test]
async fn two_graphene_engines_with_separate_roots_coexist() {
    let first_root = TempDir::new().expect("first root");
    let second_root = TempDir::new().expect("second root");
    let first = Graphene::builder(first_root.path())
        .network(local_network())
        .build()
        .await
        .expect("first engine");
    let second = Graphene::builder(second_root.path())
        .network(local_network())
        .build()
        .await
        .expect("second engine");
    assert_ne!(first.data_root(), second.data_root());
    assert!(first.data_root().join(".graphene-layout.json").is_file());
    assert!(second.data_root().join(".graphene-layout.json").is_file());
}

#[tokio::test]
async fn synthetic_nested_operation_reports_progress_and_cancels() {
    let root = TempDir::new().expect("temp root");
    let graphene = Graphene::builder(root.path())
        .network(local_network())
        .build()
        .await
        .expect("build Graphene");
    let parent = graphene.operations().create("parent");
    parent.start().expect("parent starts");
    let synthetic =
        graphene
            .operations()
            .synthetic(Some(&parent.handle()), 100, Duration::from_millis(2));
    let operation = synthetic.operation();
    let events = operation.subscribe();
    let task = tokio::spawn(async move { synthetic.await_result().await });
    tokio::time::sleep(Duration::from_millis(12)).await;
    operation.cancel();
    let error = task.await.expect("join").expect_err("cancelled");
    assert!(error.is_cancelled());
    assert_eq!(operation.parent_id(), Some(parent.handle().id()));
    assert_eq!(operation.current_state(), OperationState::Cancelled);
    assert!(
        events.try_next().is_some(),
        "synthetic operation should be observable"
    );
    let _ = parent.cancelled();
}

#[tokio::test]
async fn sha1_only_integrity_verifies_and_uses_sha1_cache_identity() {
    let server = FixtureServer::start().await;
    let root = TempDir::new().expect("temp root");
    let graphene = Graphene::builder(root.path())
        .network(local_network())
        .build()
        .await
        .expect("build Graphene");
    let request = Artifact::new(
        vec![ArtifactSource::new(server.url("/artifact"))],
        ArtifactIntegrity::none().with_sha1(FIXTURE_SHA1.parse().expect("sha1")),
    )
    .with_expected_size(fixture_bytes().len() as u64);

    let result = graphene
        .artifacts()
        .acquire(request, None)
        .await_result()
        .await
        .expect("sha1-only download succeeds");
    assert_eq!(
        result.path,
        sha1_cache_path(graphene.data_root(), FIXTURE_SHA1)
    );
    assert_eq!(result.sha1.to_string(), FIXTURE_SHA1);
    assert_eq!(
        std::fs::read(result.path).expect("cached bytes"),
        fixture_bytes()
    );
}

#[tokio::test]
async fn interrupted_body_retry_discards_partial_temp_before_retry() {
    let server = FixtureServer::start().await;
    let root = TempDir::new().expect("temp root");
    let mut network = local_network();
    network.retry_policy = RetryPolicy {
        max_attempts: 2,
        base_delay: Duration::from_millis(1),
        max_delay: Duration::from_millis(2),
    };
    let graphene = Graphene::builder(root.path())
        .network(network)
        .build()
        .await
        .expect("build Graphene");

    let result = graphene
        .artifacts()
        .acquire(artifact(server.url("/interrupted")), None)
        .await_result()
        .await
        .expect("retry after interrupted body succeeds");
    assert_eq!(server.requests("/interrupted"), 2);
    assert_eq!(
        std::fs::read(result.path).expect("final bytes"),
        fixture_bytes()
    );
    assert!(temporary_directory_is_empty(graphene.data_root()));
}

#[tokio::test]
async fn cancellation_stops_retry_delay_before_another_attempt() {
    let server = FixtureServer::start().await;
    let root = TempDir::new().expect("temp root");
    let mut network = local_network();
    network.retry_policy = RetryPolicy {
        max_attempts: 5,
        base_delay: Duration::from_millis(200),
        max_delay: Duration::from_millis(200),
    };
    let graphene = Graphene::builder(root.path())
        .network(network)
        .build()
        .await
        .expect("build Graphene");
    let prepared = graphene
        .artifacts()
        .acquire(artifact(server.url("/always-retry")), None);
    let operation = prepared.operation();
    let task = tokio::spawn(async move { prepared.await_result().await });

    wait_for_requests(&server, "/always-retry", 1).await;
    operation.cancel();
    let error = task
        .await
        .expect("task joins")
        .expect_err("retry must cancel");
    assert!(error.is_cancelled());
    assert_eq!(operation.current_state(), OperationState::Cancelled);
    tokio::time::sleep(Duration::from_millis(250)).await;
    assert_eq!(server.requests("/always-retry"), 1);
}

#[tokio::test]
async fn all_source_failure_retains_ordered_redacted_attempt_evidence() {
    let server = FixtureServer::start().await;
    let root = TempDir::new().expect("temp root");
    let mut network = local_network();
    network.retry_policy.max_attempts = 1;
    let graphene = Graphene::builder(root.path())
        .network(network)
        .build()
        .await
        .expect("build Graphene");
    let request = Artifact::new(
        vec![
            ArtifactSource::new(server.url("/permanent?token=secret-a")).with_priority(10),
            ArtifactSource::new(server.url("/permanent?token=secret-b")).with_priority(0),
        ],
        ArtifactIntegrity::none().with_sha256(FIXTURE_SHA256.parse().expect("sha256")),
    );

    let error = graphene
        .artifacts()
        .acquire(request, None)
        .await_result()
        .await
        .expect_err("all sources fail");
    let evidence = error
        .context
        .get("source_attempts")
        .expect("source attempt evidence");
    assert!(evidence.starts_with("source=1 "));
    assert!(evidence.contains("source=0 "));
    assert!(!evidence.contains("secret"));
    assert!(!evidence.contains("token"));
}

#[tokio::test]
async fn invalid_network_configuration_is_rejected_before_root_initialization() {
    let parent = TempDir::new().expect("parent");
    let root = parent.path().join("must-not-be-created");
    let mut network = local_network();
    network.proxy = graphene::ProxyPolicy::Explicit("not a proxy url".into());
    let error = Graphene::builder(&root)
        .network(network)
        .build()
        .await
        .expect_err("invalid proxy must fail");
    assert_eq!(error.code, ErrorCode::NetworkProxyInvalid);
    assert!(!root.exists());
}

#[test]
fn error_codes_serialize_to_stable_machine_values() {
    assert_eq!(
        serde_json::to_string(&ErrorCode::HashMismatch).expect("serialize error code"),
        r#""HASH_MISMATCH""#
    );
}

#[test]
fn facade_service_handles_are_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Graphene>();
    assert_send_sync::<graphene::ArtifactService>();
    assert_send_sync::<graphene::OperationService>();
    assert_send_sync::<graphene::OperationHandle>();
}

#[tokio::test]
async fn invalid_preexisting_cache_is_replaced_only_with_verified_content() {
    let server = FixtureServer::start().await;
    let root = TempDir::new().expect("temp root");
    let graphene = Graphene::builder(root.path())
        .network(local_network())
        .build()
        .await
        .expect("build Graphene");
    let destination = cache_path(graphene.data_root(), FIXTURE_SHA256);
    std::fs::create_dir_all(destination.parent().expect("parent")).expect("cache parent");
    std::fs::write(&destination, b"corrupt").expect("corrupt cache");

    let mut request = artifact(server.url("/artifact"));
    request.cache_policy = CachePolicy::UseVerified;
    let result = graphene
        .artifacts()
        .acquire(request, None)
        .await_result()
        .await
        .expect("invalid cache replaced");
    assert_eq!(result.disposition, DownloadDisposition::Downloaded);
    assert_eq!(
        std::fs::read(&destination).expect("valid final"),
        fixture_bytes()
    );
}

#[tokio::test]
async fn refresh_policy_redownloads_and_replaces_a_valid_cache_object() {
    let server = FixtureServer::start().await;
    let root = TempDir::new().expect("temp root");
    let graphene = Graphene::builder(root.path())
        .network(local_network())
        .build()
        .await
        .expect("build Graphene");

    let first = graphene
        .artifacts()
        .acquire(artifact(server.url("/artifact")), None)
        .await_result()
        .await
        .expect("first download");
    assert_eq!(first.disposition, DownloadDisposition::Downloaded);
    assert_eq!(server.requests("/artifact"), 1);

    let mut refresh = artifact(server.url("/artifact"));
    refresh.cache_policy = CachePolicy::Refresh;
    let refreshed = graphene
        .artifacts()
        .acquire(refresh, None)
        .await_result()
        .await
        .expect("refresh download");

    assert_eq!(refreshed.disposition, DownloadDisposition::Downloaded);
    assert_eq!(server.requests("/artifact"), 2);
    assert_eq!(refreshed.path, first.path);
    assert_eq!(
        std::fs::read(refreshed.path).expect("refreshed bytes"),
        fixture_bytes()
    );
    assert!(temporary_directory_is_empty(graphene.data_root()));
}
