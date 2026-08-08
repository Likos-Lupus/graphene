use graphene::{
    ErrorCode, GameEvent, Graphene, InstallRequest, InstanceId, JavaArchitecture, JavaCandidate,
    JavaCandidateSource, JavaRuntime, JavaVendor, LaunchRequest, LaunchResolution, LaunchSession,
    MinecraftVersionId, MojangProviderConfig, NetworkConfig, NewInstanceSpec, OperationEventKind,
    OperationState, ProxyPolicy, SensitiveString, probe_java,
};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex},
    time::Duration,
};
use tempfile::TempDir;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};

const VERSION_ID: &str = "1.21.1";
const SECRET: &str = "phase1-fixture-secret-token-7f45";

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn local_network() -> NetworkConfig {
    NetworkConfig {
        proxy: ProxyPolicy::None,
        max_concurrent_downloads: 4,
        ..NetworkConfig::default()
    }
}

fn local_network_with_capacity(max_concurrent_downloads: usize) -> NetworkConfig {
    NetworkConfig {
        max_concurrent_downloads,
        ..local_network()
    }
}

struct FixtureServer {
    address: std::net::SocketAddr,
    counts: Arc<Mutex<HashMap<String, usize>>>,
    behavior: FixtureBehavior,
    task: JoinHandle<()>,
}

#[derive(Clone, Default)]
struct FixtureBehavior {
    overrides: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    missing: Arc<Mutex<HashSet<String>>>,
    initial_delays: Arc<Mutex<HashMap<String, Duration>>>,
    chunk_delays: Arc<Mutex<HashMap<String, Duration>>>,
}

impl FixtureServer {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind fixture server");
        let address = listener.local_addr().expect("fixture address");
        let counts = Arc::new(Mutex::new(HashMap::new()));
        let task_counts = Arc::clone(&counts);
        let behavior = FixtureBehavior::default();
        let task_behavior = behavior.clone();
        let task = tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let counts = Arc::clone(&task_counts);
                let behavior = task_behavior.clone();
                tokio::spawn(async move {
                    let _ = serve_connection(stream, counts, behavior).await;
                });
            }
        });
        Self {
            address,
            counts,
            behavior,
            task,
        }
    }

    fn url(&self, path: &str) -> String {
        format!("http://{}{}", self.address, path)
    }

    fn requests(&self, path: &str) -> usize {
        self.counts
            .lock()
            .expect("counts")
            .get(path)
            .copied()
            .unwrap_or(0)
    }

    fn request_counts(&self) -> HashMap<String, usize> {
        self.counts.lock().expect("counts").clone()
    }

    fn override_body(&self, path: &str, body: Vec<u8>) {
        self.behavior
            .overrides
            .lock()
            .expect("overrides")
            .insert(path.to_owned(), body);
    }

    fn omit(&self, path: &str) {
        self.behavior
            .missing
            .lock()
            .expect("missing")
            .insert(path.to_owned());
    }

    fn delay(&self, path: &str, duration: Duration) {
        self.behavior
            .initial_delays
            .lock()
            .expect("delays")
            .insert(path.to_owned(), duration);
    }

    fn delay_chunks(&self, path: &str, duration: Duration) {
        self.behavior
            .chunk_delays
            .lock()
            .expect("chunk delays")
            .insert(path.to_owned(), duration);
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
    behavior: FixtureBehavior,
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
    *counts
        .lock()
        .expect("counts")
        .entry(path.clone())
        .or_default() += 1;

    let initial_delay = behavior
        .initial_delays
        .lock()
        .expect("delays")
        .get(&path)
        .copied();
    if let Some(delay) = initial_delay {
        tokio::time::sleep(delay).await;
    }
    if behavior.missing.lock().expect("missing").contains(&path) {
        return send_status(&mut stream, 404, "Not Found").await;
    }
    let override_body = behavior
        .overrides
        .lock()
        .expect("overrides")
        .get(&path)
        .cloned();
    let chunk_delay = behavior
        .chunk_delays
        .lock()
        .expect("chunk delays")
        .get(&path)
        .copied();
    if let Some(body) = override_body {
        return send_body_with_delay(&mut stream, &body, chunk_delay).await;
    }

    let fixture = if path == "/minecraft/version_manifest_v2.json" {
        Some(fixtures().join("minecraft/version_manifest_v2.json"))
    } else if let Some(name) = path.strip_prefix("/minecraft/") {
        if name.contains('/') || name.contains('\\') || name == "." || name == ".." {
            None
        } else {
            Some(fixtures().join("minecraft").join(name))
        }
    } else if let Some(name) = path.strip_prefix("/artifacts/") {
        Some(fixtures().join("artifacts").join(name))
    } else if let Some(name) = path.strip_prefix("/native-jars/") {
        Some(fixtures().join("native-jars").join(name))
    } else if path.starts_with("/assets/") {
        Some(fixtures().join("artifacts/fixture.ogg"))
    } else {
        None
    };

    match fixture {
        Some(path) if path.is_file() => {
            send_body_with_delay(&mut stream, &fs::read(path)?, chunk_delay).await
        }
        _ => send_status(&mut stream, 404, "Not Found").await,
    }
}

async fn send_body_with_delay(
    stream: &mut TcpStream,
    body: &[u8],
    chunk_delay: Option<Duration>,
) -> std::io::Result<()> {
    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes()).await?;
    let chunk_size = if chunk_delay.is_some() { 1 } else { 4096 };
    for chunk in body.chunks(chunk_size) {
        stream.write_all(chunk).await?;
        stream.flush().await?;
        tokio::task::yield_now().await;
        if let Some(delay) = chunk_delay {
            tokio::time::sleep(delay).await;
        }
    }
    Ok(())
}

async fn send_status(stream: &mut TcpStream, code: u16, reason: &str) -> std::io::Result<()> {
    let response =
        format!("HTTP/1.1 {code} {reason}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
    stream.write_all(response.as_bytes()).await
}

async fn fixture_graphene(root: &Path, server: &FixtureServer) -> Graphene {
    fixture_graphene_with_network(root, server, local_network()).await
}

async fn fixture_graphene_with_network(
    root: &Path,
    server: &FixtureServer,
    network: NetworkConfig,
) -> Graphene {
    let provider = MojangProviderConfig::fixture(
        server.url("/minecraft/version_manifest_v2.json"),
        server.url("/assets"),
    )
    .expect("fixture provider config");
    Graphene::builder(root)
        .network(network)
        .minecraft_provider(provider)
        .build()
        .await
        .expect("Graphene fixture engine")
}

fn install_request(id_byte: u8, name: &str) -> InstallRequest {
    let id = InstanceId::from_bytes([id_byte; 16]);
    let instance = NewInstanceSpec::with_id(id, name).expect("instance spec");
    InstallRequest::with_instance(
        instance,
        MinecraftVersionId::new(VERSION_ID).expect("version ID"),
    )
}

async fn install_fixture(graphene: &Graphene, id_byte: u8, name: &str) -> InstanceId {
    let request = install_request(id_byte, name);
    let id = request.instance.id;
    let plan = graphene
        .install()
        .plan(request)
        .await_result()
        .await
        .expect("install plan");
    assert_eq!(plan.minecraft.version_id.as_str(), VERSION_ID);
    assert_eq!(plan.minecraft.java_requirement.major_version, 21);
    assert_eq!(plan.minecraft.assets.objects.len(), 1);
    assert!(
        plan.artifacts.len() >= 7,
        "client/library/native/asset metadata must be planned"
    );
    graphene
        .install()
        .execute(plan)
        .await_result()
        .await
        .expect("install execute");
    id
}

async fn wait_for_request(server: &FixtureServer, path: &str) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while server.requests(path) == 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("fixture request was not observed");
}

fn assert_no_committed_or_staged_instance(graphene: &Graphene, id: InstanceId) {
    let instances = graphene.data_root().join("instances");
    assert!(
        !instances.join(id.to_string()).exists(),
        "failed/cancelled install must not publish target"
    );
    let staging = instances.join(".staging");
    if let Ok(entries) = fs::read_dir(staging) {
        let prefix = format!("{}-", id);
        for entry in entries.flatten() {
            assert!(
                !entry.file_name().to_string_lossy().starts_with(&prefix),
                "staging directory must be cleaned"
            );
        }
    }
}

async fn cancel_execution_at_stage(
    graphene: &Graphene,
    plan: graphene::InstallPlan,
    stage: &str,
) -> (ErrorCode, OperationState) {
    let id = plan.instance.descriptor.instance_id;
    let prepared = graphene.install().execute(plan);
    let handle = prepared.operation();
    let events = handle.subscribe();
    let task = tokio::spawn(prepared.await_result());
    tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(event) = events.next().await {
            if matches!(&event.kind, OperationEventKind::StageChanged { stage: observed } if observed == stage) {
                handle.cancel();
                return;
            }
        }
        panic!("operation ended before cancellation stage {stage}");
    })
    .await
    .expect("stage cancellation timeout");
    let error = task
        .await
        .expect("install task join")
        .expect_err("cancelled install");
    let state = handle.current_state();
    assert_no_committed_or_staged_instance(graphene, id);
    (error.code, state)
}

fn compile_fake_java(output_dir: &Path) -> PathBuf {
    compile_fake_java_named(output_dir, "fake-java")
}

fn compile_fake_java_named(output_dir: &Path, name: &str) -> PathBuf {
    let source = fixtures().join("java/fake_java.rs");
    let executable_name = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    };
    let executable = output_dir.join(executable_name);
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let output = Command::new(rustc)
        .arg(&source)
        .arg("-O")
        .arg("-o")
        .arg(&executable)
        .output()
        .expect("execute rustc for fake Java helper");
    assert!(
        output.status.success(),
        "fake Java compilation failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    executable
}

fn fixture_session() -> LaunchSession {
    LaunchSession {
        username: "FixturePlayer".to_owned(),
        uuid: "0123456789abcdef0123456789abcdef".to_owned(),
        access_token: SensitiveString::new(SECRET),
        user_type: "msa".to_owned(),
        client_id: Some(SensitiveString::new("fixture-client-id-secret")),
        xuid: Some(SensitiveString::new("fixture-xuid-secret")),
    }
}

fn normalize_native_classifier(value: &str) -> String {
    ["natives-linux", "natives-windows", "natives-osx"]
        .into_iter()
        .fold(value.to_owned(), |value, classifier| {
            value.replace(classifier, "<native-classifier>")
        })
}

fn minecraft_arguments_snapshot(arguments: &[graphene::Argument]) -> Value {
    Value::Array(
        arguments
            .iter()
            .map(|argument| match argument {
                graphene::Argument::Literal(value) => json!({"values": [value]}),
                graphene::Argument::Conditional { rules, values } => {
                    json!({"values": values, "rule_count": rules.len()})
                }
                _ => json!({"unsupported_future_variant": true}),
            })
            .collect(),
    )
}

fn installed_arguments_snapshot(arguments: &[graphene::InstalledArgument]) -> Value {
    Value::Array(
        arguments
            .iter()
            .map(|argument| match argument {
                graphene::InstalledArgument::Literal(value) => json!({"values": [value]}),
                graphene::InstalledArgument::Conditional { rules, values } => {
                    json!({"values": values, "rule_count": rules.len()})
                }
                _ => json!({"unsupported_future_variant": true}),
            })
            .collect(),
    )
}

fn resolved_snapshot(value: &graphene::ResolvedMinecraft) -> Value {
    let libraries = value
        .libraries
        .iter()
        .map(|library| {
            let coordinate = format!(
                "{}:{}:{}",
                library.coordinate.group, library.coordinate.artifact, library.coordinate.version
            );
            json!({
                "coordinate": coordinate,
                "classpath": library.classpath_artifact.as_ref().map(|artifact| artifact.relative_path.as_str()),
                "native": library.native_artifact.as_ref().map(|artifact| normalize_native_classifier(artifact.relative_path.as_str())),
            })
        })
        .collect::<Vec<_>>();
    let assets = value
        .assets
        .objects
        .iter()
        .map(|asset| json!({"name": asset.logical_name, "hash": asset.hash, "size": asset.size, "path": asset.artifact.relative_path.as_str()}))
        .collect::<Vec<_>>();
    json!({
        "version_id": value.version_id.as_str(),
        "version_type": format!("{:?}", value.version_type),
        "main_class": value.main_class.as_str(),
        "java_major": value.java_requirement.major_version,
        "java_component": value.java_requirement.component_hint.as_deref(),
        "client": value.client.relative_path.as_str(),
        "libraries": libraries,
        "asset_index_id": value.assets.index_id.as_str(),
        "asset_index": value.assets.index.relative_path.as_str(),
        "assets": assets,
        "logging": value.logging.as_ref().map(|logging| json!({"path": logging.artifact.relative_path.as_str(), "argument": logging.argument.as_str()})),
        "jvm_arguments": minecraft_arguments_snapshot(&value.jvm_args),
        "game_arguments": minecraft_arguments_snapshot(&value.game_args),
    })
}

fn install_plan_snapshot(value: &graphene::InstallPlan) -> Value {
    json!({
        "plan_version": value.plan_version,
        "instance_id": value.instance.descriptor.instance_id.to_string(),
        "display_name": value.instance.descriptor.display_name.as_str(),
        "relative_root": value.instance.relative_root.as_str(),
        "requested_version": value.requested_version.as_str(),
        "artifact_sizes": value.artifacts.iter().map(|entry| entry.artifact.expected_size).collect::<Vec<_>>(),
        "shared_destinations": value.shared_materializations.iter().map(|entry| normalize_native_classifier(entry.destination.as_str())).collect::<Vec<_>>(),
        "instance_destinations": value.instance_materializations.iter().map(|entry| entry.destination.as_str()).collect::<Vec<_>>(),
        "native_destinations": value.native_extractions.iter().map(|entry| entry.destination.as_str()).collect::<Vec<_>>(),
        "metadata_paths": value.metadata_artifacts.iter().map(|entry| entry.relative_path.as_str()).collect::<Vec<_>>(),
    })
}

fn receipt_snapshot(value: &graphene::InstallReceipt) -> Value {
    json!({
        "schema_version": value.schema_version,
        "install_format_version": value.install_format_version,
        "instance_id": value.instance_id.to_string(),
        "requested_version": value.requested_version.as_str(),
        "resolved_version": value.resolved_version.as_str(),
        "version_type": value.version_type.as_str(),
        "main_class": value.main_class.as_str(),
        "java_major": value.java_requirement.major_version,
        "java_component": value.java_requirement.component_hint.as_deref(),
        "client": value.client.path.as_str(),
        "libraries": value.libraries.iter().map(|library| json!({
            "coordinate": library.coordinate.as_str(),
            "classpath": library.classpath.as_ref().map(|artifact| artifact.path.as_str()),
            "native": library.native_archive.as_ref().map(|artifact| normalize_native_classifier(artifact.path.as_str())),
        })).collect::<Vec<_>>(),
        "asset_index_id": value.asset_index_id.as_str(),
        "asset_index": value.asset_index.path.as_str(),
        "assets_root": value.assets_root.as_str(),
        "natives_directory": value.natives_directory.as_str(),
        "logging": value.logging_configuration.as_ref().map(|artifact| artifact.path.as_str()),
        "logging_argument": value.logging_argument.as_deref(),
        "jvm_arguments": installed_arguments_snapshot(&value.jvm_arguments),
        "game_arguments": installed_arguments_snapshot(&value.game_arguments),
    })
}

fn normalize_launch_snapshot(mut value: Value) -> Value {
    if let Some(object) = value.as_object_mut() {
        object.insert("java".to_owned(), Value::String("<java>".to_owned()));
        object.insert(
            "classpath_separator".to_owned(),
            Value::String("<platform-classpath-separator>".to_owned()),
        );
        if let Some(arguments) = object.get_mut("jvm_args").and_then(Value::as_array_mut) {
            for argument in arguments {
                if argument
                    .as_str()
                    .is_some_and(|value| value.starts_with("-Dfixture.os="))
                {
                    *argument = Value::String("-Dfixture.os=<platform>".to_owned());
                }
            }
        }
    }
    value
}

fn assert_json_snapshot(actual: Value, expected: &str) {
    let expected: Value = serde_json::from_str(expected).expect("checked snapshot JSON");
    assert_eq!(actual, expected);
}

#[tokio::test]
async fn phase1_fixture_resolves_installs_reuses_cache_and_plans_offline_launch() {
    let server = FixtureServer::start().await;
    let root = TempDir::new().expect("root");
    let graphene = fixture_graphene(root.path(), &server).await;

    let manifest = graphene
        .minecraft()
        .versions()
        .await_result()
        .await
        .expect("manifest");
    assert_eq!(manifest.versions.len(), 1);
    assert_eq!(manifest.versions[0].id.as_str(), VERSION_ID);
    let missing = manifest
        .select(&MinecraftVersionId::new("latest").expect("id"))
        .expect_err("explicit missing version");
    assert_eq!(missing.code, ErrorCode::MinecraftVersionNotFound);

    let first = install_fixture(&graphene, 0x11, "Fixture One").await;
    let first_root = graphene
        .data_root()
        .join("instances")
        .join(first.to_string());
    assert!(first_root.join("instance.json").is_file());
    assert!(first_root.join(".graphene/install.json").is_file());
    assert!(
        first_root
            .join(".minecraft/versions/1.21.1/1.21.1.jar")
            .is_file()
    );
    assert!(
        first_root
            .join(".graphene/natives/1.21.1/native/fixture-native.bin")
            .is_file()
    );

    let artifact_counts = [
        "/minecraft/1.21.1.json",
        "/minecraft/1.21.1-assets.json",
        "/artifacts/client.jar",
        "/artifacts/fixture-lib-1.0.jar",
        "/native-jars/fixture-native.jar",
        "/artifacts/log4j2.xml",
    ]
    .map(|path| (path, server.requests(path)));
    assert!(artifact_counts.iter().all(|(_, count)| *count == 1));

    let _second = install_fixture(&graphene, 0x22, "Fixture Two").await;
    for (path, before) in artifact_counts {
        assert_eq!(
            server.requests(path),
            before,
            "verified artifact should be reused: {path}"
        );
    }
    // The unhashed top-level manifest is intentionally reacquired.
    assert!(server.requests("/minecraft/version_manifest_v2.json") >= 3);

    let fake = compile_fake_java(root.path());
    let candidate = JavaCandidate {
        executable: fake.clone(),
        source: JavaCandidateSource::Explicit,
    };
    let probed = probe_java(&candidate).await.expect("fake Java probe");
    assert_eq!(probed.major_version, 21);
    assert_eq!(probed.architecture, JavaArchitecture::current());

    let request = LaunchRequest {
        resolution: Some(LaunchResolution {
            width: 1280,
            height: 720,
        }),
        ..LaunchRequest::new(first, fixture_session())
    };
    let runtime = JavaRuntime {
        executable: fake,
        version: "21.0.4".to_owned(),
        major_version: 21,
        vendor: JavaVendor::Other("Graphene Fixture Vendor".to_owned()),
        architecture: JavaArchitecture::current(),
        java_home: None,
    };
    let plan = graphene
        .launch()
        .plan_with_java(request, runtime)
        .await
        .expect("offline launch plan");
    let debug = format!("{plan:?}");
    let snapshot = serde_json::to_string_pretty(&plan.redacted_snapshot(graphene.data_root()))
        .expect("snapshot");
    assert!(!debug.contains(SECRET));
    assert!(!snapshot.contains(SECRET));
    assert!(snapshot.contains("<secret>"));
    assert!(
        !snapshot.contains("${"),
        "Tier A launch plan must have no raw placeholders"
    );
}

#[tokio::test]
async fn phase1_public_launch_plan_selects_explicit_java_without_network() {
    let server = FixtureServer::start().await;
    let root = TempDir::new().expect("root");
    let graphene = fixture_graphene(root.path(), &server).await;
    let instance = install_fixture(&graphene, 0x23, "Public Launch Plan").await;
    let fake = compile_fake_java(root.path());

    let selected = graphene
        .java()
        .select_for_instance(instance, Some(fake.clone()))
        .await
        .expect("explicit fake Java selection");
    assert_eq!(selected.executable, fake);
    assert_eq!(selected.major_version, 21);
    assert_eq!(selected.architecture, JavaArchitecture::current());

    let before = server.request_counts();
    let mut request = LaunchRequest::new(instance, fixture_session());
    request.java_override = Some(fake.clone());
    request.resolution = Some(LaunchResolution {
        width: 1280,
        height: 720,
    });
    let plan = graphene
        .launch()
        .plan(request)
        .await
        .expect("public launch service plan");
    let after = server.request_counts();

    assert_eq!(plan.java.executable, fake);
    assert_eq!(plan.java.major_version, 21);
    assert_eq!(plan.java.architecture, JavaArchitecture::current());
    assert_eq!(
        after, before,
        "launch planning must perform no network requests"
    );

    let debug = format!("{plan:?}");
    let snapshot = serde_json::to_string(&plan.redacted_snapshot(graphene.data_root()))
        .expect("redacted launch snapshot");
    assert!(!debug.contains(SECRET));
    assert!(!snapshot.contains(SECRET));
    assert!(snapshot.contains("<secret>"));
}

#[tokio::test]
async fn phase1_semantic_snapshots_are_stable_and_secret_safe() {
    let server = FixtureServer::start().await;
    let root = TempDir::new().expect("root");
    let graphene = fixture_graphene(root.path(), &server).await;
    let request = install_request(0x71, "Snapshot Fixture");
    let instance = request.instance.id;
    let plan = graphene
        .install()
        .plan(request)
        .await_result()
        .await
        .expect("snapshot install plan");

    assert_json_snapshot(
        resolved_snapshot(&plan.minecraft),
        include_str!("snapshots/resolved_minecraft.json"),
    );
    assert_json_snapshot(
        install_plan_snapshot(&plan),
        include_str!("snapshots/install_plan.json"),
    );
    assert_json_snapshot(
        receipt_snapshot(&plan.receipt),
        include_str!("snapshots/install_receipt.json"),
    );

    graphene
        .install()
        .execute(plan)
        .await_result()
        .await
        .expect("snapshot install execute");
    let fake = compile_fake_java(root.path());
    let runtime = JavaRuntime {
        executable: fake,
        version: "21.0.4".to_owned(),
        major_version: 21,
        vendor: JavaVendor::Other("Snapshot Fixture".to_owned()),
        architecture: JavaArchitecture::current(),
        java_home: None,
    };
    let request = LaunchRequest {
        resolution: Some(LaunchResolution {
            width: 1280,
            height: 720,
        }),
        ..LaunchRequest::new(instance, fixture_session())
    };
    let launch = graphene
        .launch()
        .plan_with_java(request, runtime)
        .await
        .expect("snapshot launch plan");
    let snapshot = normalize_launch_snapshot(
        serde_json::to_value(launch.redacted_snapshot(graphene.data_root()))
            .expect("redacted launch snapshot"),
    );
    let rendered = serde_json::to_string(&snapshot).expect("render redacted snapshot");
    assert!(!rendered.contains(SECRET));
    assert_json_snapshot(
        snapshot,
        include_str!("snapshots/launch_plan_redacted.json"),
    );
}

#[tokio::test]
async fn phase1_fake_java_receives_secret_emits_bounded_events_and_supports_exit_and_kill() {
    let server = FixtureServer::start().await;
    let root = TempDir::new().expect("root");
    let graphene = fixture_graphene(root.path(), &server).await;
    let instance = install_fixture(&graphene, 0x33, "Launch Fixture").await;
    let fake = compile_fake_java(root.path());
    let runtime = JavaRuntime {
        executable: fake,
        version: "21.0.4".to_owned(),
        major_version: 21,
        vendor: JavaVendor::Other("Fixture".to_owned()),
        architecture: JavaArchitecture::current(),
        java_home: None,
    };

    let capture = root.path().join("captured-argv.txt");
    let mut request = LaunchRequest::new(instance, fixture_session());
    request.extra_game_args.push("--fake-exit=7".to_owned());
    let mut plan = graphene
        .launch()
        .plan_with_java(request, runtime.clone())
        .await
        .expect("plan");
    plan.environment.values.insert(
        "GRAPHENE_FAKE_JAVA_CAPTURE".to_owned(),
        capture.to_string_lossy().into_owned(),
    );
    let game = graphene.launch().execute(plan).expect("spawn fake Java");
    assert!(game.pid() > 0);
    let mut events = game.take_events().expect("single event stream");
    let mut saw_start = false;
    let mut saw_stdout = false;
    let mut saw_stderr = false;
    while let Some(event) = tokio::time::timeout(Duration::from_secs(5), events.next())
        .await
        .expect("event timeout")
    {
        match event {
            GameEvent::Started { pid } => {
                assert_eq!(pid, game.pid());
                saw_start = true;
            }
            GameEvent::Stdout { text, .. } => {
                if text.contains("fixture stdout") {
                    saw_stdout = true;
                }
            }
            GameEvent::Stderr { text, .. } => {
                if text.contains("fixture stderr") {
                    saw_stderr = true;
                }
            }
            GameEvent::Exited { .. } => break,
            GameEvent::Failed { code } => panic!("unexpected process failure: {code}"),
            _ => {}
        }
    }
    let exit = game
        .wait()
        .await
        .expect("non-zero exit is observable, not transport failure");
    assert!(!exit.success);
    assert_eq!(exit.code, Some(7));
    assert!(!exit.killed);
    assert!(saw_start && saw_stdout && saw_stderr);
    let child_argv = fs::read_to_string(&capture).expect("captured child argv");
    assert!(
        child_argv.contains(SECRET),
        "secret must reach the child process argv"
    );
    assert!(!format!("{game:?}").contains(SECRET));

    let mut kill_request = LaunchRequest::new(instance, fixture_session());
    kill_request.extra_game_args.push("--fake-sleep".to_owned());
    let kill_plan = graphene
        .launch()
        .plan_with_java(kill_request, runtime)
        .await
        .expect("kill plan");
    let killed_game = graphene
        .launch()
        .execute(kill_plan)
        .expect("spawn sleeping fake Java");
    tokio::time::sleep(Duration::from_millis(50)).await;
    killed_game.kill().await.expect("kill");
    let killed = tokio::time::timeout(Duration::from_secs(5), killed_game.wait())
        .await
        .expect("kill wait timeout")
        .expect("kill wait");
    assert!(killed.killed);
}

#[tokio::test]
async fn phase1_fake_java_probe_timeout_and_malformed_output_are_structured() {
    let root = TempDir::new().expect("root");
    let timeout = compile_fake_java_named(root.path(), "fake-java-timeout");
    let timeout_candidate = JavaCandidate {
        executable: timeout,
        source: JavaCandidateSource::Explicit,
    };
    assert_eq!(
        probe_java(&timeout_candidate)
            .await
            .expect_err("probe must time out")
            .code,
        ErrorCode::JavaProbeTimeout
    );

    let malformed = compile_fake_java_named(root.path(), "fake-java-malformed");
    let malformed_candidate = JavaCandidate {
        executable: malformed,
        source: JavaCandidateSource::Explicit,
    };
    assert_eq!(
        probe_java(&malformed_candidate)
            .await
            .expect_err("probe must reject malformed output")
            .code,
        ErrorCode::JavaProbeFailed
    );
}

#[tokio::test]
async fn phase1_failure_matrix_is_structured_and_never_publishes_partial_instances() {
    // Malformed top-level manifest is parsed under the bounded metadata GET path.
    {
        let server = FixtureServer::start().await;
        server.override_body("/minecraft/version_manifest_v2.json", b"{".to_vec());
        let root = TempDir::new().expect("root");
        let graphene = fixture_graphene(root.path(), &server).await;
        let request = install_request(0x41, "Bad Manifest");
        let id = request.instance.id;
        let error = graphene
            .install()
            .plan(request)
            .await_result()
            .await
            .expect_err("malformed manifest");
        assert_eq!(error.code, ErrorCode::MinecraftManifestInvalid);
        assert_no_committed_or_staged_instance(&graphene, id);
    }

    // A syntactically malformed version document with a matching declared digest reaches the
    // provider parser instead of being rejected as an integrity mismatch first.
    {
        let server = FixtureServer::start().await;
        server.override_body(
            "/minecraft/version_manifest_v2.json",
            fs::read(fixtures().join("minecraft/version_manifest_malformed_version.json"))
                .expect("malformed-version manifest"),
        );
        let root = TempDir::new().expect("root");
        let graphene = fixture_graphene(root.path(), &server).await;
        let request = install_request(0x42, "Bad Version JSON");
        let id = request.instance.id;
        let error = graphene
            .install()
            .plan(request)
            .await_result()
            .await
            .expect_err("malformed version JSON");
        assert_eq!(error.code, ErrorCode::MinecraftMetadataInvalid);
        assert_no_committed_or_staged_instance(&graphene, id);
    }

    // Asset-index body corruption is rejected by the shared Phase 0 verifier before parsing.
    {
        let server = FixtureServer::start().await;
        server.override_body("/minecraft/1.21.1-assets.json", vec![b'X'; 148]);
        let root = TempDir::new().expect("root");
        let graphene = fixture_graphene(root.path(), &server).await;
        let request = install_request(0x43, "Bad Asset Index");
        let id = request.instance.id;
        let error = graphene
            .install()
            .plan(request)
            .await_result()
            .await
            .expect_err("asset index integrity mismatch");
        assert_eq!(error.code, ErrorCode::HashMismatch);
        assert_no_committed_or_staged_instance(&graphene, id);
    }

    // Client hash mismatch during execution cannot publish a target.
    {
        let server = FixtureServer::start().await;
        let root = TempDir::new().expect("root");
        let graphene = fixture_graphene(root.path(), &server).await;
        let request = install_request(0x44, "Bad Client");
        let id = request.instance.id;
        let plan = graphene
            .install()
            .plan(request)
            .await_result()
            .await
            .expect("plan");
        server.override_body("/artifacts/client.jar", vec![b'X'; 35]);
        let error = graphene
            .install()
            .execute(plan)
            .await_result()
            .await
            .expect_err("client integrity mismatch");
        assert_eq!(error.code, ErrorCode::HashMismatch);
        assert_no_committed_or_staged_instance(&graphene, id);
    }

    // A missing library is a transport failure, not a partially valid instance.
    {
        let server = FixtureServer::start().await;
        let root = TempDir::new().expect("root");
        let graphene = fixture_graphene(root.path(), &server).await;
        let request = install_request(0x45, "Missing Library");
        let id = request.instance.id;
        let plan = graphene
            .install()
            .plan(request)
            .await_result()
            .await
            .expect("plan");
        server.omit("/artifacts/fixture-lib-1.0.jar");
        let error = graphene
            .install()
            .execute(plan)
            .await_result()
            .await
            .expect_err("missing library");
        assert_eq!(error.code, ErrorCode::NetworkStatusError);
        assert_no_committed_or_staged_instance(&graphene, id);
    }

    // A malicious but correctly hashed native JAR reaches archive validation and is rejected.
    {
        let server = FixtureServer::start().await;
        server.override_body(
            "/minecraft/version_manifest_v2.json",
            fs::read(fixtures().join("minecraft/version_manifest_traversal.json"))
                .expect("traversal manifest"),
        );
        let root = TempDir::new().expect("root");
        let graphene = fixture_graphene(root.path(), &server).await;
        let request = install_request(0x46, "Traversal Native");
        let id = request.instance.id;
        let plan = graphene
            .install()
            .plan(request)
            .await_result()
            .await
            .expect("plan");
        let error = graphene
            .install()
            .execute(plan)
            .await_result()
            .await
            .expect_err("native traversal");
        assert_eq!(error.code, ErrorCode::InstallNativeExtractionFailed);
        assert_no_committed_or_staged_instance(&graphene, id);
    }

    // Existing shared destination with the wrong filesystem type causes materialization failure.
    {
        let server = FixtureServer::start().await;
        let root = TempDir::new().expect("root");
        let graphene = fixture_graphene(root.path(), &server).await;
        let request = install_request(0x47, "Materialization Failure");
        let id = request.instance.id;
        let plan = graphene
            .install()
            .plan(request)
            .await_result()
            .await
            .expect("plan");
        fs::create_dir_all(
            graphene
                .data_root()
                .join("shared/libraries/com/example/fixture-lib/1.0/fixture-lib-1.0.jar"),
        )
        .expect("conflicting materialization directory");
        let error = graphene
            .install()
            .execute(plan)
            .await_result()
            .await
            .expect_err("materialization failure");
        assert_eq!(error.code, ErrorCode::InstallStageFailed);
        assert_no_committed_or_staged_instance(&graphene, id);
    }

    // Environmental mutation of staged content is detected before the create-only commit.
    {
        let server = FixtureServer::start().await;
        let root = TempDir::new().expect("root");
        let graphene = fixture_graphene(root.path(), &server).await;
        let request = install_request(0x49, "Staged Validation Failure");
        let id = request.instance.id;
        let plan = graphene
            .install()
            .plan(request)
            .await_result()
            .await
            .expect("plan");
        let prepared = graphene.install().execute(plan);
        let handle = prepared.operation();
        let events = handle.subscribe();
        let operation_id = handle.id();
        let task = tokio::spawn(prepared.await_result());
        tokio::time::timeout(Duration::from_secs(5), async {
            while let Some(event) = events.next().await {
                if matches!(&event.kind, OperationEventKind::StageChanged { stage } if stage == "validate-staging") {
                    let staged_client = graphene
                        .data_root()
                        .join("instances/.staging")
                        .join(format!("{id}-{operation_id}"))
                        .join(".minecraft/versions/1.21.1/1.21.1.jar");
                    fs::remove_file(staged_client).expect("remove staged client before validation");
                    return;
                }
            }
            panic!("install ended before staged validation was observed");
        })
        .await
        .expect("staged validation observation timeout");
        let error = task
            .await
            .expect("staged validation join")
            .expect_err("staged validation failure");
        assert_eq!(error.code, ErrorCode::InstallValidationFailed);
        assert_no_committed_or_staged_instance(&graphene, id);
    }

    // Target collision is detected before provider/network work begins.
    {
        let server = FixtureServer::start().await;
        let root = TempDir::new().expect("root");
        let graphene = fixture_graphene(root.path(), &server).await;
        let request = install_request(0x48, "Collision");
        let id = request.instance.id;
        fs::create_dir(graphene.data_root().join("instances").join(id.to_string()))
            .expect("collision target");
        let error = graphene
            .install()
            .plan(request)
            .await_result()
            .await
            .expect_err("target collision");
        assert_eq!(error.code, ErrorCode::InstallTargetExists);
        assert_eq!(server.requests("/minecraft/version_manifest_v2.json"), 0);
    }
}

#[tokio::test]
async fn phase1_cancellation_during_metadata_request_is_terminal_and_clean() {
    let server = FixtureServer::start().await;
    server.delay(
        "/minecraft/version_manifest_v2.json",
        Duration::from_secs(2),
    );
    let root = TempDir::new().expect("root");
    let graphene = fixture_graphene(root.path(), &server).await;
    let request = install_request(0x51, "Cancel Metadata");
    let id = request.instance.id;
    let prepared = graphene.install().plan(request);
    let handle = prepared.operation();
    let task = tokio::spawn(prepared.await_result());
    wait_for_request(&server, "/minecraft/version_manifest_v2.json").await;
    handle.cancel();
    let error = task
        .await
        .expect("plan task join")
        .expect_err("cancelled metadata request");
    assert_eq!(error.code, ErrorCode::InstallCancelled);
    assert_eq!(handle.current_state(), OperationState::Cancelled);
    assert_no_committed_or_staged_instance(&graphene, id);
}

#[tokio::test]
async fn phase1_cancellation_while_waiting_for_download_capacity_is_terminal() {
    let server = FixtureServer::start().await;
    server.delay(
        "/minecraft/version_manifest_v2.json",
        Duration::from_millis(600),
    );
    let root = TempDir::new().expect("root");
    let graphene =
        fixture_graphene_with_network(root.path(), &server, local_network_with_capacity(1)).await;

    let first = graphene
        .install()
        .plan(install_request(0x52, "Capacity Holder"));
    let first_task = tokio::spawn(first.await_result());
    wait_for_request(&server, "/minecraft/version_manifest_v2.json").await;

    let second_request = install_request(0x53, "Capacity Waiter");
    let second_id = second_request.instance.id;
    let second = graphene.install().plan(second_request);
    let second_handle = second.operation();
    let second_task = tokio::spawn(second.await_result());
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert_eq!(
        server.requests("/minecraft/version_manifest_v2.json"),
        1,
        "second request should still be waiting for the semaphore"
    );
    second_handle.cancel();
    let error = second_task
        .await
        .expect("second plan join")
        .expect_err("capacity wait cancellation");
    assert_eq!(error.code, ErrorCode::InstallCancelled);
    assert_eq!(second_handle.current_state(), OperationState::Cancelled);
    assert_no_committed_or_staged_instance(&graphene, second_id);
    let _ = first_task.await.expect("first plan join");
}

#[tokio::test]
async fn phase1_cancellation_during_asset_transfer_and_transaction_stages_is_clean() {
    // Cancel during the artifact batch while the first client artifact is still in-flight.
    {
        let server = FixtureServer::start().await;
        let root = TempDir::new().expect("root");
        let graphene = fixture_graphene(root.path(), &server).await;
        let request = install_request(0x57, "Cancel Artifact Batch");
        let id = request.instance.id;
        let plan = graphene
            .install()
            .plan(request)
            .await_result()
            .await
            .expect("plan");
        server.delay_chunks("/artifacts/client.jar", Duration::from_millis(30));
        let prepared = graphene.install().execute(plan);
        let handle = prepared.operation();
        let task = tokio::spawn(prepared.await_result());
        wait_for_request(&server, "/artifacts/client.jar").await;
        tokio::time::sleep(Duration::from_millis(60)).await;
        handle.cancel();
        let error = task
            .await
            .expect("artifact batch cancellation join")
            .expect_err("artifact batch cancellation");
        assert_eq!(error.code, ErrorCode::InstallCancelled);
        assert_eq!(handle.current_state(), OperationState::Cancelled);
        assert_no_committed_or_staged_instance(&graphene, id);
    }

    // Cancel while an asset object's HTTP body is actively being streamed.
    {
        let server = FixtureServer::start().await;
        let root = TempDir::new().expect("root");
        let graphene = fixture_graphene(root.path(), &server).await;
        let request = install_request(0x54, "Cancel Asset");
        let id = request.instance.id;
        let plan = graphene
            .install()
            .plan(request)
            .await_result()
            .await
            .expect("plan");
        let asset_path = "/assets/2e/2e33cf671c7dfe5f545e01ee202ad3caf8e55da1";
        server.delay_chunks(asset_path, Duration::from_millis(30));
        let prepared = graphene.install().execute(plan);
        let handle = prepared.operation();
        let task = tokio::spawn(prepared.await_result());
        wait_for_request(&server, asset_path).await;
        tokio::time::sleep(Duration::from_millis(60)).await;
        handle.cancel();
        let error = task
            .await
            .expect("asset cancellation join")
            .expect_err("asset transfer cancellation");
        assert_eq!(error.code, ErrorCode::InstallCancelled);
        assert_eq!(handle.current_state(), OperationState::Cancelled);
        assert_no_committed_or_staged_instance(&graphene, id);
    }

    // The extraction stage observes the operation token before publication.
    {
        let server = FixtureServer::start().await;
        let root = TempDir::new().expect("root");
        let graphene = fixture_graphene(root.path(), &server).await;
        let plan = graphene
            .install()
            .plan(install_request(0x55, "Cancel Native"))
            .await_result()
            .await
            .expect("plan");
        let (code, state) = cancel_execution_at_stage(&graphene, plan, "extract-natives").await;
        assert_eq!(code, ErrorCode::InstallCancelled);
        assert_eq!(state, OperationState::Cancelled);
    }

    // The explicit pre-commit yield is the final cancellable boundary before publication seals.
    {
        let server = FixtureServer::start().await;
        let root = TempDir::new().expect("root");
        let graphene = fixture_graphene(root.path(), &server).await;
        let plan = graphene
            .install()
            .plan(install_request(0x56, "Cancel Precommit"))
            .await_result()
            .await
            .expect("plan");
        let (code, state) = cancel_execution_at_stage(&graphene, plan, "pre-commit").await;
        assert_eq!(code, ErrorCode::InstallCancelled);
        assert_eq!(state, OperationState::Cancelled);
    }
}

#[tokio::test]
async fn phase1_slow_event_consumer_cannot_deadlock_child_output() {
    let server = FixtureServer::start().await;
    let root = TempDir::new().expect("root");
    let graphene = fixture_graphene(root.path(), &server).await;
    let instance = install_fixture(&graphene, 0x61, "Backpressure Fixture").await;
    let fake = compile_fake_java(root.path());
    let runtime = JavaRuntime {
        executable: fake,
        version: "21.0.4".to_owned(),
        major_version: 21,
        vendor: JavaVendor::Other("Fixture".to_owned()),
        architecture: JavaArchitecture::current(),
        java_home: None,
    };
    let mut request = LaunchRequest::new(instance, fixture_session());
    request.extra_game_args.push("--fake-spam=20000".to_owned());
    let plan = graphene
        .launch()
        .plan_with_java(request, runtime)
        .await
        .expect("backpressure plan");
    let game = graphene_launch::execute(plan, 4).expect("spawn fake Java with tiny event queue");

    // Deliberately do not consume events until after the process exits. The OS pipes must still be
    // drained by independent tasks and host-facing output must be dropped rather than blocking.
    let exit = tokio::time::timeout(Duration::from_secs(10), game.wait())
        .await
        .expect("slow consumer must not deadlock child")
        .expect("process wait");
    assert!(exit.success);
    assert!(
        game.dropped_output_count() > 0,
        "bounded queue should report dropped output"
    );
    let mut events = game.take_events().expect("event stream");
    let mut saw_terminal = false;
    while let Some(event) = events.next().await {
        if matches!(event, GameEvent::Exited { .. }) {
            saw_terminal = true;
        }
    }
    assert!(
        saw_terminal,
        "terminal event must survive output backpressure"
    );
}
