use graphene::{
    AccountKind, AccountState, AuthInteraction, Graphene, MicrosoftAuthConfig,
    MicrosoftServiceEndpoints, NetworkConfig, OfflineAccountSpec, OperationEventKind, ProxyPolicy,
};
use graphene_auth::{InMemorySecretStore, SecretRecordIdentity, SecretStore};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fmt::Write as _,
    fs,
    sync::{Arc, Mutex},
    time::Duration,
};
use tempfile::TempDir;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};

const REFRESH: &str = "PHASE2_REFRESH_TOKEN_DO_NOT_PRINT";
const ACCESS: &str = "PHASE2_ACCESS_TOKEN_DO_NOT_PRINT";
const DEVICE: &str = "PHASE2_DEVICE_CODE_DO_NOT_PRINT";
const XSTS: &str = "PHASE2_XSTS_TOKEN_DO_NOT_PRINT";
const MC: &str = "PHASE2_MC_TOKEN_DO_NOT_PRINT";
const PROFILE_ID: &str = "01234567-89ab-cdef-0123-456789abcdef";

fn local_network() -> NetworkConfig {
    NetworkConfig {
        proxy: ProxyPolicy::None,
        connect_timeout: Duration::from_secs(2),
        request_timeout: Duration::from_secs(5),
        ..NetworkConfig::default()
    }
}

#[derive(Default)]
struct AuthFixtureState {
    device_polls: usize,
    refreshes: usize,
    requests: HashMap<String, usize>,
}

struct AuthFixtureServer {
    address: std::net::SocketAddr,
    state: Arc<Mutex<AuthFixtureState>>,
    task: JoinHandle<()>,
}

impl AuthFixtureServer {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind auth fixture");
        let address = listener.local_addr().expect("fixture address");
        let state = Arc::new(Mutex::new(AuthFixtureState::default()));
        let service_state = Arc::clone(&state);
        let task = tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    break;
                };
                let connection_state = Arc::clone(&service_state);
                tokio::spawn(async move {
                    let _ = serve_auth_connection(stream, connection_state).await;
                });
            }
        });

        Self {
            address,
            state,
            task,
        }
    }

    fn base(&self) -> String {
        format!("http://{}", self.address)
    }

    fn config(&self) -> MicrosoftAuthConfig {
        let base = self.base();
        MicrosoftAuthConfig::fixture(
            "phase2-fixture-client",
            vec!["XboxLive.signin".into(), "offline_access".into()],
            format!("{base}/devicecode"),
            format!("{base}/token"),
            MicrosoftServiceEndpoints {
                xbox_user_auth: format!("{base}/xbox"),
                xsts_authorize: format!("{base}/xsts"),
                minecraft_login: format!("{base}/minecraft/login"),
                minecraft_entitlements: format!("{base}/minecraft/entitlements"),
                minecraft_profile: format!("{base}/minecraft/profile"),
                xbox_relying_party: "http://auth.xboxlive.com".into(),
                xsts_relying_party: "rp://api.minecraftservices.com/".into(),
            },
        )
        .expect("valid fixture auth config")
    }

    fn request_count(&self, path: &str) -> usize {
        self.state
            .lock()
            .expect("fixture state")
            .requests
            .get(path)
            .copied()
            .unwrap_or(0)
    }
}

impl Drop for AuthFixtureServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn serve_auth_connection(
    mut stream: TcpStream,
    state: Arc<Mutex<AuthFixtureState>>,
) -> std::io::Result<()> {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 2048];
    let header_end = loop {
        let read = stream.read(&mut buffer).await?;
        if read == 0 {
            return Ok(());
        }

        request.extend_from_slice(&buffer[..read]);
        if let Some(position) = request.windows(4).position(|window| window == b"\r\n\r\n") {
            break position + 4;
        }

        if request.len() > 64 * 1024 {
            return Ok(());
        }
    };

    let headers = String::from_utf8_lossy(&request[..header_end]);
    let request_line = headers.lines().next().unwrap_or_default();
    let mut request_parts = request_line.split_whitespace();
    let _method = request_parts.next().unwrap_or_default();
    let path = request_parts.next().unwrap_or("/").to_owned();
    let content_length = headers
        .lines()
        .find_map(|line| line.split_once(':'))
        .filter(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.trim().parse::<usize>().ok())
        .unwrap_or(0);

    while request.len() < header_end + content_length {
        let read = stream.read(&mut buffer).await?;
        if read == 0 {
            break;
        }

        request.extend_from_slice(&buffer[..read]);
    }

    let body = String::from_utf8_lossy(request.get(header_end..).unwrap_or_default());

    let (status, response) = {
        let mut state = state.lock().expect("fixture state");
        *state.requests.entry(path.clone()).or_insert(0) += 1;
        match path.as_str() {
            "/devicecode" => (
                200,
                format!(
                    r#"{{"device_code":"{DEVICE}","user_code":"ABCD-EFGH","verification_uri":"https://example.invalid/device","expires_in":30,"interval":1,"message":"Enter the displayed fixture code"}}"#,
                ),
            ),
            "/token" if body.contains("device_code") => {
                state.device_polls += 1;
                if state.device_polls == 1 {
                    (400, r#"{"error":"authorization_pending"}"#.to_owned())
                } else {
                    (200, format!(r#"{{"access_token":"{ACCESS}","refresh_token":"{REFRESH}"}}"#))
                }
            }
            "/token" if body.contains("refresh_token") => {
                state.refreshes += 1;
                (200, format!(r#"{{"access_token":"{ACCESS}-rotated","refresh_token":"{REFRESH}-rotated"}}"#))
            }
            "/xbox" => (
                200,
                r#"{"Token":"PHASE2_XBOX_TOKEN_DO_NOT_PRINT","DisplayClaims":{"xui":[{"uhs":"fixture-uhs","xid":"fixture-xuid"}]}}"#.to_owned(),
            ),
            "/xsts" => (
                200,
                format!(r#"{{"Token":"{XSTS}","DisplayClaims":{{"xui":[{{"uhs":"fixture-uhs","xid":"fixture-xuid"}}]}}}}"#),
            ),
            "/minecraft/login" => (200, format!(r#"{{"access_token":"{MC}"}}"#)),
            "/minecraft/entitlements" => (200, r#"{"items":[{"name":"game_minecraft"}]}"#.to_owned()),
            "/minecraft/profile" => (
                200,
                format!(r#"{{"id":"{PROFILE_ID}","name":"FixturePlayer"}}"#),
            ),
            _ => (404, r#"{"error":"not_found"}"#.to_owned()),
        }
    };

    let reason = if status == 200 {
        "OK"
    } else if status == 400 {
        "Bad Request"
    } else {
        "Not Found"
    };

    let response_bytes = response.as_bytes();
    let wire = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        response_bytes.len()
    );

    stream.write_all(wire.as_bytes()).await?;
    stream.write_all(response_bytes).await?;
    stream.shutdown().await
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
    }
    encoded
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn offline_account_persists_uuid_across_restart_and_converges_to_launch_session() {
    let root = TempDir::new().expect("temp root");
    let first = Graphene::builder(root.path())
        .network(local_network())
        .build()
        .await
        .expect("engine");
    let account = first
        .accounts()
        .create_offline(OfflineAccountSpec::new("Local_Player"))
        .await
        .expect("offline account");

    assert_eq!(account.kind, AccountKind::Offline);
    assert_eq!(account.state, AccountState::Ready);

    let uuid = account.profile.minecraft_uuid;
    drop(first);

    let second = Graphene::builder(root.path())
        .network(local_network())
        .build()
        .await
        .expect("restart engine");
    let persisted = second
        .accounts()
        .get(account.id)
        .await
        .expect("get")
        .expect("persisted account");

    assert_eq!(persisted.profile.minecraft_uuid, uuid);

    let session = second
        .accounts()
        .launch_session(account.id)
        .await_result()
        .await
        .expect("offline launch session");

    assert_eq!(session.username, "Local_Player");
    assert_eq!(session.uuid, uuid.to_string());
    assert_eq!(session.access_token.expose_secret(), "0");

    second.accounts().remove(account.id).await.expect("remove");
    assert!(
        second
            .accounts()
            .get(account.id)
            .await
            .expect("get removed")
            .is_none()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn microsoft_fixture_login_restart_refresh_rotation_and_secret_regression() {
    let root = TempDir::new().expect("temp root");
    let server = AuthFixtureServer::start().await;
    let secrets = Arc::new(InMemorySecretStore::new_for_tests());
    let engine = Graphene::builder(root.path())
        .network(local_network())
        .microsoft_auth(server.config())
        .secret_store(secrets.clone())
        .build()
        .await
        .expect("engine");

    let login = engine
        .accounts()
        .begin_microsoft_login()
        .await
        .expect("begin login");
    match login.interaction() {
        AuthInteraction::DeviceAuthorization { user_code, .. } => {
            assert_eq!(user_code.expose_secret(), "ABCD-EFGH");
            assert!(!format!("{:?}", login.interaction()).contains("ABCD-EFGH"));
        }
        _ => panic!("unexpected auth interaction"),
    }

    let operation = login.operation();
    let events = operation.subscribe();
    let collector = tokio::spawn(async move {
        let events = events;
        let mut rendered = String::new();
        while let Some(event) = events.next().await {
            rendered.push_str(&format!("{event:?}\n"));
            if matches!(
                event.kind,
                OperationEventKind::Completed
                    | OperationEventKind::Failed { .. }
                    | OperationEventKind::Cancelled
            ) {
                break;
            }
        }
        rendered
    });

    let account = login.await_result().await.expect("complete login");
    assert_eq!(account.kind, AccountKind::Microsoft);
    assert_eq!(account.state, AccountState::Ready);

    let rendered_events = collector.await.expect("event collector");
    assert!(rendered_events.contains("authenticate_xbox"));
    assert!(rendered_events.contains("verify_entitlement"));

    let identity = SecretRecordIdentity::microsoft_refresh(account.id);
    assert_eq!(
        secrets
            .get(&identity)
            .expect("secret read")
            .expect("refresh secret")
            .expose_secret(),
        REFRESH
    );

    let account_path = root
        .path()
        .join("config/accounts")
        .join(format!("{}.json", account.id));
    let account_json = fs::read_to_string(&account_path).expect("account JSON");
    for secret in [
        REFRESH,
        ACCESS,
        DEVICE,
        XSTS,
        MC,
        "PHASE2_XBOX_TOKEN_DO_NOT_PRINT",
    ] {
        assert!(
            !account_json.contains(secret),
            "account JSON leaked fixture secret"
        );
        assert!(
            !rendered_events.contains(secret),
            "operation events leaked fixture secret"
        );
    }

    assert!(!format!("{account:?}").contains(REFRESH));
    drop(engine);

    let restarted = Graphene::builder(root.path())
        .network(local_network())
        .microsoft_auth(server.config())
        .secret_store(secrets.clone())
        .build()
        .await
        .expect("restart engine");
    let refresh_one = restarted.accounts().launch_session(account.id);
    let refresh_two = restarted.accounts().launch_session(account.id);
    let (session_one, session_two) =
        tokio::join!(refresh_one.await_result(), refresh_two.await_result());
    let session = session_one.expect("first refreshed launch session");
    let second_session = session_two.expect("second refreshed launch session");

    assert_eq!(session.username, "FixturePlayer");
    assert_eq!(session.uuid, PROFILE_ID);
    assert_eq!(session.access_token.expose_secret(), MC);
    assert!(!format!("{session:?}").contains(MC));
    assert!(!format!("{second_session:?}").contains(MC));
    assert_eq!(second_session.uuid, PROFILE_ID);

    let rotated_refresh = format!("{REFRESH}-rotated");
    assert_eq!(
        secrets
            .get(&identity)
            .expect("secret read")
            .expect("rotated refresh")
            .expose_secret(),
        rotated_refresh.as_str()
    );
    assert!(
        server.request_count("/token") >= 4,
        "concurrent refreshes must serialize without losing rotation"
    );
    restarted
        .accounts()
        .remove(account.id)
        .await
        .expect("remove account");
    assert!(secrets.get(&identity).expect("secret read").is_none());
}

struct JavaFixtureServer {
    address: std::net::SocketAddr,
    archive: Arc<Vec<u8>>,
    checksum: String,
    requests: Arc<Mutex<HashMap<String, usize>>>,
    task: JoinHandle<()>,
}

impl JavaFixtureServer {
    async fn start(archive: Vec<u8>, checksum: String) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind Java fixture");
        let address = listener.local_addr().expect("Java fixture address");
        let archive = Arc::new(archive);
        let requests = Arc::new(Mutex::new(HashMap::new()));
        let service_archive = Arc::clone(&archive);
        let service_requests = Arc::clone(&requests);
        let task = tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    break;
                };
                let archive = Arc::clone(&service_archive);
                let requests = Arc::clone(&service_requests);
                tokio::spawn(async move {
                    let _ = serve_java_connection(stream, address, archive, requests).await;
                });
            }
        });

        Self {
            address,
            archive,
            checksum,
            requests,
            task,
        }
    }

    fn provider_config(&self) -> graphene::AdoptiumProviderConfig {
        graphene::AdoptiumProviderConfig::fixture(format!("http://{}/v3", self.address))
            .expect("valid Java fixture config")
    }

    fn count_matching(&self, prefix: &str) -> usize {
        self.requests
            .lock()
            .expect("Java fixture requests")
            .iter()
            .filter(|(path, _)| path.starts_with(prefix))
            .map(|(_, count)| *count)
            .sum()
    }
}

impl Drop for JavaFixtureServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn serve_java_connection(
    mut stream: TcpStream,
    address: std::net::SocketAddr,
    archive: Arc<Vec<u8>>,
    requests: Arc<Mutex<HashMap<String, usize>>>,
) -> std::io::Result<()> {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 2048];

    loop {
        let read = stream.read(&mut buffer).await?;
        if read == 0 {
            return Ok(());
        }

        request.extend_from_slice(&buffer[..read]);
        if request.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }

        if request.len() > 64 * 1024 {
            return Ok(());
        }
    }

    let request_text = String::from_utf8_lossy(&request);
    let path = request_text
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/")
        .to_owned();
    *requests
        .lock()
        .expect("Java fixture requests")
        .entry(path.clone())
        .or_insert(0) += 1;

    let (status, content_type, body) = if path == "/runtime.zip" {
        (200, "application/zip", archive.as_ref().clone())
    } else if path.starts_with("/v3/assets/latest/") {
        let major = path
            .trim_start_matches("/v3/assets/latest/")
            .split('/')
            .next()
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(21);
        let architecture = match std::env::consts::ARCH {
            "x86_64" => "x64",
            "x86" | "i686" => "x86",
            "aarch64" => "aarch64",
            other => other,
        };
        let os = match std::env::consts::OS {
            "macos" => "mac",
            other => other,
        };
        let digest = sha256_hex(archive.as_ref());
        let metadata = serde_json::json!([{
            "binary": {
                "architecture": architecture,
                "image_type": "jre",
                "os": os,
                "package": {
                    "checksum": digest,
                    "link": format!("http://{address}/runtime.zip"),
                    "name": "OpenJDK21U-jre-fixture.zip",
                    "size": archive.len()
                }
            },
            "release_name": format!("jdk-{major}.0.4+fixture"),
            "vendor": "Eclipse Adoptium",
            "version": { "major": major, "semver": format!("{major}.0.4+7") }
        }]);
        (
            200,
            "application/json",
            serde_json::to_vec(&metadata).expect("Java metadata JSON"),
        )
    } else {
        (
            404,
            "application/json",
            br#"{"error":"not_found"}"#.to_vec(),
        )
    };

    let reason = if status == 200 { "OK" } else { "Not Found" };
    let header = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );

    stream.write_all(header.as_bytes()).await?;
    stream.write_all(&body).await?;
    stream.shutdown().await
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320u32 & (0u32.wrapping_sub(crc & 1)));
        }
    }
    !crc
}

fn push_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn stored_zip(entry_name: &str, data: &[u8]) -> Vec<u8> {
    let name = entry_name.as_bytes();
    let size = u32::try_from(data.len()).expect("fixture archive fits u32");
    let crc = crc32(data);
    let mut out = Vec::new();

    push_u32(&mut out, 0x0403_4b50);
    push_u16(&mut out, 20);
    push_u16(&mut out, 0);
    push_u16(&mut out, 0);
    push_u16(&mut out, 0);
    push_u16(&mut out, 0);
    push_u32(&mut out, crc);
    push_u32(&mut out, size);
    push_u32(&mut out, size);
    push_u16(&mut out, name.len() as u16);
    push_u16(&mut out, 0);

    out.extend_from_slice(name);
    out.extend_from_slice(data);

    let central_offset = out.len() as u32;

    push_u32(&mut out, 0x0201_4b50);
    push_u16(&mut out, 20);
    push_u16(&mut out, 20);
    push_u16(&mut out, 0);
    push_u16(&mut out, 0);
    push_u16(&mut out, 0);
    push_u16(&mut out, 0);
    push_u32(&mut out, crc);
    push_u32(&mut out, size);
    push_u32(&mut out, size);
    push_u16(&mut out, name.len() as u16);
    push_u16(&mut out, 0);
    push_u16(&mut out, 0);
    push_u16(&mut out, 0);
    push_u16(&mut out, 0);
    push_u32(&mut out, 0);
    push_u32(&mut out, 0);

    out.extend_from_slice(name);

    let central_size = out.len() as u32 - central_offset;

    push_u32(&mut out, 0x0605_4b50);
    push_u16(&mut out, 0);
    push_u16(&mut out, 0);
    push_u16(&mut out, 1);
    push_u16(&mut out, 1);
    push_u32(&mut out, central_size);
    push_u32(&mut out, central_offset);
    push_u16(&mut out, 0);

    out
}

fn compile_fake_java(root: &std::path::Path) -> Vec<u8> {
    let source =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/java/fake_java.rs");
    let executable = root.join(if cfg!(windows) { "java.exe" } else { "java" });
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let output = std::process::Command::new(rustc)
        .arg(source)
        .arg("-O")
        .arg("-o")
        .arg(&executable)
        .output()
        .expect("compile fake Java");

    assert!(
        output.status.success(),
        "fake Java compilation failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::read(executable).expect("read fake Java")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn managed_java_fixture_installs_probes_commits_restarts_and_reuses_cache() {
    if !matches!(std::env::consts::ARCH, "x86" | "x86_64" | "aarch64")
        || !matches!(std::env::consts::OS, "windows" | "linux" | "macos")
    {
        return;
    }

    let root = TempDir::new().expect("temp root");
    let helper = TempDir::new().expect("helper root");
    let executable = compile_fake_java(helper.path());
    let name = if cfg!(windows) {
        "fixture-jre/bin/java.exe"
    } else {
        "fixture-jre/bin/java"
    };
    let archive = stored_zip(name, &executable);
    let checksum = sha256_hex(&archive);
    let server = JavaFixtureServer::start(archive, checksum.clone()).await;

    assert_eq!(server.checksum, checksum);
    assert!(!server.archive.is_empty());

    let engine = Graphene::builder(root.path())
        .network(local_network())
        .managed_java_provider(server.provider_config())
        .build()
        .await
        .expect("engine");
    let requirement = graphene::JavaRequirement {
        major_version: 21,
        component_hint: None,
    };
    let first = engine
        .java()
        .install_managed(requirement.clone())
        .await_result()
        .await
        .expect("managed Java install");

    assert_eq!(first.major_version, 21);
    assert!(
        first
            .executable
            .starts_with(root.path().join("shared/runtimes"))
    );

    let inventory = engine
        .java()
        .managed_runtimes()
        .await
        .expect("managed inventory");

    assert_eq!(inventory.len(), 1);
    assert_eq!(inventory[0].major_version, 21);
    assert_eq!(inventory[0].archive_sha256.to_string(), checksum);

    let first_downloads = server.count_matching("/runtime.zip");
    assert_eq!(first_downloads, 1);

    let repeated = engine
        .java()
        .install_managed(requirement.clone())
        .await_result()
        .await
        .expect("managed Java reuse");

    assert_eq!(repeated.executable, first.executable);
    assert_eq!(
        server.count_matching("/runtime.zip"),
        first_downloads,
        "committed runtime should avoid another archive download"
    );

    drop(engine);

    let restarted = Graphene::builder(root.path())
        .network(local_network())
        .managed_java_provider(server.provider_config())
        .build()
        .await
        .expect("restart engine");
    let inventory = restarted
        .java()
        .managed_runtimes()
        .await
        .expect("restart inventory");

    assert_eq!(inventory.len(), 1);
    assert_eq!(inventory[0].major_version, 21);

    let descriptor = fs::read_to_string(
        root.path()
            .join("shared/runtimes")
            .join(inventory[0].id.to_string())
            .join("runtime.json"),
    )
    .expect("runtime descriptor");

    for secret in [REFRESH, ACCESS, DEVICE, XSTS, MC] {
        assert!(!descriptor.contains(secret));
    }
}
