use anyhow::{Context, Result, anyhow, bail};
use blnk::config::args::{ConnectArgs, CpArgs, DevicesArgs, ServeArgs, WebArgs};
use blnk::config::{Config, DeviceRegistry};
use blnk::identity::Identity;
use blnk::peer::TwoPeerHarness;
use blnk::protocol::swsp::DEFAULT_MAX_PAYLOAD_LEN;
use blnk::session::{SessionRuntime, SessionRuntimeConfig};
use blnk::signaling::EndpointPolicy;
use blnk::signaling::orchestration::{
    DEFAULT_ORCHESTRATION_TIMEOUT, accept_server_session, connect_target, run_file_client,
    run_shell_client, serve_session_with_shutdown,
};
use blnk::stream::file::{
    FileOperation, FileTransferCancellation, FileTransferConfig, FileTransferRequest,
    FileTransferResponse, FileTransferService, collect_data_frames, decode_request_frame,
    decode_response_metadata, encode_response_frames,
};
use blnk::stream::shell::{
    ShellCommand, ShellPolicy, ShellStreamHandler, decode_error_frame, decode_exit_frame,
    decode_open_frame, decode_output_frame,
};
use blnk::web::{BrowserControlConfig, BrowserControlServer};
use clap::{Parser, Subcommand};
use std::io::Write;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use tokio_util::sync::CancellationToken;

const LOCAL_FIXTURE_DEVICE_ID: &str = "local-fixture";
const DEFAULT_FIXTURE_PIN: &str = "123456";

#[derive(Debug, Parser)]
#[command(name = "blnk", version, about = "Remote access multitool")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Start the local service and initialize its persistent identity.
    Serve(ServeArgs),
    /// Connect to a peer or device. Use --local-fixture for a secret-free local session.
    Connect(ConnectArgs),
    /// Copy a local file to or from the local authenticated fixture.
    Cp(CpArgs),
    /// List metadata for known devices; private keys and access codes are not shown.
    Devices(DevicesArgs),
    /// Start the loopback-only browser control surface.
    Web(WebArgs),
    /// Print the installed version.
    Version,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Serve(args) => run_serve(args).await,
        Commands::Connect(args) => run_connect(args).await,
        Commands::Cp(args) => run_cp(args).await,
        Commands::Devices(args) => run_devices(args).await,
        Commands::Web(args) => run_web(args).await,
        Commands::Version => {
            println!("blnk {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
    }
}

async fn run_serve(args: ServeArgs) -> Result<()> {
    let config = Config::load().context("load blnk configuration")?;
    let signaling_url = args
        .signaling_url
        .unwrap_or_else(|| config.signaling_url.clone());
    let identity = load_or_create_identity(&config.identity_path)?;

    println!("identity_uid={}", identity.uid());
    println!("signaling_endpoint_configured=true");

    if args.local_fixture {
        let pin = args
            .pin
            .or(config.pin.clone())
            .unwrap_or_else(|| identity.pairing_code().to_owned());
        let fixture = blnk::signaling::LocalFixtureServer::start_with_config(
            blnk::signaling::FixtureConfig {
                pairing_code: pin.clone(),
                ..Default::default()
            },
        )
        .await
        .context("start local signaling fixture")?;
        println!("fixture_url={}", fixture.url());
        println!("fixture_pin_configured=true");
        if args.once {
            fixture.shutdown().await.context("stop local fixture")?;
        } else {
            println!("serve_status=running; press Ctrl-C to stop");
            tokio::signal::ctrl_c()
                .await
                .context("wait for shutdown signal")?;
            fixture.shutdown().await.context("stop local fixture")?;
        }
        return Ok(());
    }

    let pin = args
        .pin
        .or(config.pin)
        .ok_or_else(|| anyhow!("remote serve requires --pin or BLNK_PIN"))?;
    let session = accept_server_session(
        &signaling_url,
        &identity,
        pin,
        EndpointPolicy::PublicOnly,
        DEFAULT_ORCHESTRATION_TIMEOUT,
    )
    .await
    .context("establish remote signaling/WebRTC session")?;
    println!("serve_status=connected; client_id={}", session.client_id);
    let root = std::env::current_dir().context("get serve root")?;
    if args.once {
        blnk::signaling::orchestration::serve_session(session, root)
            .await
            .context("serve remote session")?;
        return Ok(());
    }

    println!("serve_status=running; press Ctrl-C to stop");
    let shutdown = CancellationToken::new();
    let session_task = serve_session_with_shutdown(session, root, shutdown.clone());
    tokio::pin!(session_task);
    tokio::select! {
        result = &mut session_task => {
            result.context("serve remote session")?;
        }
        signal = tokio::signal::ctrl_c() => {
            signal.context("wait for shutdown signal")?;
            shutdown.cancel();
            session_task.await.context("serve remote session")?;
        }
    }
    Ok(())
}

async fn run_web(args: WebArgs) -> Result<()> {
    let bootstrap_token = args
        .bootstrap_token
        .or_else(|| std::env::var("BLNK_WEB_TOKEN").ok())
        .ok_or_else(|| anyhow!("web requires --bootstrap-token or BLNK_WEB_TOKEN"))?;
    let bind_addr = SocketAddr::new(args.host, args.port);
    let config =
        BrowserControlConfig::loopback(args.origin, bootstrap_token).with_bind_addr(bind_addr);
    let mut server = BrowserControlServer::start(config)
        .await
        .context("start browser control surface")?;
    println!("browser_control_url={}", server.url());
    println!("browser_control_origin_configured=true");
    println!("browser_control_status=running; press Ctrl-C to stop");
    tokio::signal::ctrl_c()
        .await
        .context("wait for shutdown signal")?;
    server
        .shutdown()
        .await
        .context("stop browser control surface")
}

async fn run_connect(args: ConnectArgs) -> Result<()> {
    let config = Config::load().context("load blnk configuration")?;
    if args.local_fixture {
        let pin = args
            .pin
            .or(config.pin.clone())
            .unwrap_or_else(|| DEFAULT_FIXTURE_PIN.to_owned());
        let command = shell_command_from_args(&args.command)?;
        let result = run_local_shell(&pin, command).await?;
        remember_fixture(&config)?;
        if result != 0 {
            bail!("remote shell exited with status {result}");
        }
        return Ok(());
    }

    let target = args
        .target
        .as_deref()
        .ok_or_else(|| anyhow!("connect requires --target or --local-fixture"))?;
    let registry = DeviceRegistry::load(&config.devices_path).context("load device registry")?;
    ensure_known_device(&registry, target)?;
    let device = registry
        .find(target)
        .ok_or_else(|| anyhow!("device '{target}' disappeared from the registry"))?;
    let identity = load_or_create_identity(&config.identity_path)?;
    let pin = args
        .pin
        .or(config.pin)
        .ok_or_else(|| anyhow!("connect to a remote device requires --pin or BLNK_PIN"))?;
    let mut session = connect_target(
        &device.endpoint,
        &identity,
        target,
        pin,
        EndpointPolicy::PublicOnly,
        DEFAULT_ORCHESTRATION_TIMEOUT,
    )
    .await
    .context("establish remote signaling/WebRTC session")?;
    let command = shell_command_from_args(&args.command)?;
    let result = run_shell_client(&mut session.runtime, &command).await;
    let close_result = session.runtime.close().await;
    let exit_code = match (result, close_result) {
        (Err(error), _) => return Err(error.into()),
        (Ok(_), Err(error)) => return Err(error.into()),
        (Ok(code), Ok(())) => code,
    };
    if exit_code != 0 {
        bail!("remote shell exited with status {exit_code}");
    }
    Ok(())
}

async fn run_cp(args: CpArgs) -> Result<()> {
    let config = Config::load().context("load blnk configuration")?;
    if args.local_fixture {
        run_local_cp(&args.source, &args.destination, args.overwrite).await?;
        remember_fixture(&config)?;
        return Ok(());
    }

    let target = args
        .target
        .as_deref()
        .ok_or_else(|| anyhow!("remote cp requires --target or --local-fixture"))?;
    let registry = DeviceRegistry::load(&config.devices_path).context("load device registry")?;
    ensure_known_device(&registry, target)?;
    let device = registry
        .find(target)
        .ok_or_else(|| anyhow!("device '{target}' disappeared from the registry"))?;
    let identity = load_or_create_identity(&config.identity_path)?;
    let pin = args
        .pin
        .or(config.pin)
        .ok_or_else(|| anyhow!("remote cp requires --pin or BLNK_PIN"))?;
    let mut session = connect_target(
        &device.endpoint,
        &identity,
        target,
        pin,
        EndpointPolicy::PublicOnly,
        DEFAULT_ORCHESTRATION_TIMEOUT,
    )
    .await
    .context("establish remote signaling/WebRTC session")?;
    let result = run_file_client(
        &mut session.runtime,
        &args.source,
        &args.destination,
        args.overwrite,
    )
    .await;
    let close_result = session.runtime.close().await;
    match (result, close_result) {
        (Err(error), _) => Err(error.into()),
        (Ok(()), Err(error)) => Err(error.into()),
        (Ok(()), Ok(())) => Ok(()),
    }
}

async fn run_devices(args: DevicesArgs) -> Result<()> {
    let config = Config::load().context("load blnk configuration")?;
    let mut registry =
        DeviceRegistry::load(&config.devices_path).context("load device registry")?;

    if args.scan {
        println!(
            "Scanning local network for blnk peers (timeout: {}s)...",
            args.timeout
        );
        let discovered = blnk::utils::discovery::discover_local_peers(
            std::time::Duration::from_secs(args.timeout),
        )
        .await
        .context("scan local peers via mDNS")?;

        if discovered.is_empty() {
            println!("No local peers discovered.");
        } else {
            println!("Discovered {} local peer(s):", discovered.len());
            println!("ID\tENDPOINT");
            for peer in &discovered {
                println!("{}\t{}", peer.id, peer.endpoint);
                registry.upsert(&peer.id, &peer.endpoint);
            }
            registry
                .save(&config.devices_path)
                .context("save discovered peers to registry")?;
        }
        return Ok(());
    }

    if registry.devices.is_empty() {
        println!("No devices registered.");
        return Ok(());
    }

    if args.list || !registry.devices.is_empty() {
        println!("ID\tENDPOINT\tLAST_SEEN_UNIX");
        for device in registry.devices {
            println!(
                "{}\t{}\t{}",
                device.id, device.endpoint, device.last_seen_unix
            );
        }
    }
    Ok(())
}

fn load_or_create_identity(path: &str) -> Result<Identity> {
    let path = Path::new(path);
    if path.exists() {
        return Identity::load(path).map_err(Into::into);
    }
    let identity = Identity::generate().context("generate identity")?;
    identity
        .save(path)
        .with_context(|| format!("save identity to {}", path.display()))?;
    Ok(identity)
}

fn ensure_known_device(registry: &DeviceRegistry, target: &str) -> Result<()> {
    if registry.find(target).is_none() {
        bail!("unknown device '{target}'; run `blnk devices --list` or use --local-fixture");
    }
    Ok(())
}

fn remember_fixture(config: &Config) -> Result<()> {
    let mut registry =
        DeviceRegistry::load(&config.devices_path).context("load device registry")?;
    registry.upsert(LOCAL_FIXTURE_DEVICE_ID, "in-process://loopback");
    registry
        .save(&config.devices_path)
        .context("save device registry")?;
    Ok(())
}

async fn fixture_pair(pin: &str) -> Result<(SessionRuntime, SessionRuntime)> {
    let harness = TwoPeerHarness::new("control")
        .await
        .context("create local WebRTC fixture")?;
    let mut client = SessionRuntime::new(
        harness.offerer,
        SessionRuntimeConfig::client(pin.to_owned()),
    )
    .context("create fixture client runtime")?;
    let mut server = SessionRuntime::new(
        harness.answerer,
        SessionRuntimeConfig::server(pin.to_owned()),
    )
    .context("create fixture server runtime")?;

    let (client_result, server_result) = tokio::join!(client.handshake(), server.handshake());
    client_result.context("complete fixture client handshake")?;
    server_result.context("complete fixture server handshake")?;
    Ok((client, server))
}

async fn run_local_shell(pin: &str, command: ShellCommand) -> Result<i32> {
    let (mut client, mut server) = fixture_pair(pin).await?;
    let stream = client
        .open_shell_stream("/")
        .context("open local shell stream")?;
    let stream_id = stream.id();
    client
        .send_shell_open(stream_id, &command)
        .await
        .context("send shell open")?;

    let open_frame = server
        .recv_shell_frame()
        .await
        .context("receive shell open")?;
    let server_command = decode_open_frame(&open_frame).context("decode shell open")?;
    server
        .accept_shell_stream(stream_id, "/")
        .context("accept shell stream")?;

    let root = std::env::current_dir().context("get shell fixture root")?;
    let policy = ShellPolicy::new(root).context("create shell policy")?;
    let handler = ShellStreamHandler::new(policy);
    let result = handler
        .run(server_command, CancellationToken::new())
        .await
        .context("execute bounded shell command")?;
    if !result.stdout.is_empty() {
        server
            .send_shell_output(stream_id, result.stdout.clone(), false)
            .await
            .context("send shell stdout")?;
    }
    if !result.stderr.is_empty() {
        server
            .send_shell_output(stream_id, result.stderr.clone(), false)
            .await
            .context("send shell stderr")?;
    }
    server
        .send_shell_exit(stream_id, result.exit_code.unwrap_or(-1), result.signaled)
        .await
        .context("send shell exit")?;

    loop {
        let frame = client
            .recv_shell_frame()
            .await
            .context("receive shell result")?;
        if frame.flags.is_fin() {
            if let Ok(exit) = decode_exit_frame(&frame) {
                let _ = client.close_shell_stream(stream_id).await;
                let _ = server.close().await;
                let _ = client.close().await;
                return Ok(exit.code);
            }
            let error = decode_error_frame(&frame)
                .map(|message| message.message)
                .unwrap_or_else(|_| "remote shell failed".to_owned());
            let _ = server.close().await;
            let _ = client.close().await;
            bail!("remote shell error: {error}");
        }
        let output = decode_output_frame(&frame).context("decode shell output")?;
        client_output(&output.data)?;
    }
}

async fn run_local_cp(source: &str, destination: &str, overwrite: bool) -> Result<()> {
    let fixture_root = std::env::temp_dir().join(format!("blnk-fixture-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&fixture_root).context("create file fixture root")?;
    let result = run_local_cp_inner(&fixture_root, source, destination, overwrite).await;
    let _ = std::fs::remove_dir_all(&fixture_root);
    result
}

async fn run_local_cp_inner(
    fixture_root: &Path,
    source: &str,
    destination: &str,
    overwrite: bool,
) -> Result<()> {
    let is_download = source.strip_prefix("remote:");
    let (request, upload, local_destination) = if let Some(remote_path) = is_download {
        (
            FileTransferRequest::get(remote_path, None).context("create download request")?,
            Vec::new(),
            Some(PathBuf::from(destination)),
        )
    } else {
        let data =
            std::fs::read(source).with_context(|| format!("read local source {}", source))?;
        (
            FileTransferRequest::put(destination, data.len() as u64, overwrite),
            data,
            None,
        )
    };

    let service = FileTransferService::new(
        FileTransferConfig::new(fixture_root).context("create receiver file service")?,
    );
    let (mut client, mut server) = fixture_pair(DEFAULT_FIXTURE_PIN).await?;
    let stream = client
        .open_file_stream("/")
        .context("open local file stream")?;
    let stream_id = stream.id();
    client
        .send_file_request(stream_id, &request)
        .await
        .context("send file request")?;

    let open_frame = server
        .recv_file_frame()
        .await
        .context("receive file request")?;
    let received_request = decode_request_frame(&open_frame).context("decode file request")?;
    server
        .accept_file_stream(stream_id, "/")
        .context("accept file stream")?;

    if received_request.operation == FileOperation::Put {
        if upload.is_empty() {
            client
                .send_file_chunk(stream_id, Vec::new(), true)
                .await
                .context("send empty file chunk")?;
        } else {
            for (index, chunk) in upload.chunks(DEFAULT_MAX_PAYLOAD_LEN).enumerate() {
                client
                    .send_file_chunk(
                        stream_id,
                        chunk.to_vec(),
                        (index + 1) * DEFAULT_MAX_PAYLOAD_LEN >= upload.len(),
                    )
                    .await
                    .context("send file chunk")?;
            }
        }
    }

    let mut body_frames = Vec::new();
    if received_request.operation == FileOperation::Put {
        loop {
            let frame = server
                .recv_file_frame()
                .await
                .context("receive file chunk")?;
            let final_chunk = frame.flags.is_fin();
            body_frames.push(frame);
            if final_chunk {
                break;
            }
        }
    }
    let upload_body = collect_data_frames(
        body_frames,
        received_request.size,
        service.config().max_file_size,
        &FileTransferCancellation::default(),
    )
    .context("collect file upload")?;
    let response = service
        .execute(
            received_request.clone(),
            &upload_body,
            &FileTransferCancellation::default(),
        )
        .await
        .context("execute receiver file operation")?;
    for frame in encode_response_frames(stream_id, &response).context("encode file response")? {
        server
            .send_file_chunk(stream_id, frame.payload, frame.flags.is_fin())
            .await
            .context("send file response")?;
    }

    let mut received_data = Vec::new();
    let mut metadata_seen = false;
    loop {
        let frame = client
            .recv_file_frame()
            .await
            .context("receive file response")?;
        if !metadata_seen {
            let metadata = decode_response_metadata(&request, &frame)
                .context("decode file response metadata")?;
            metadata_seen = true;
            if let FileTransferResponse::Download { info, .. } = metadata {
                println!("received={} bytes={}", info.name, info.size);
            }
        } else if request.operation == FileOperation::Get {
            received_data.extend_from_slice(&frame.payload);
        }
        if frame.flags.is_fin() {
            break;
        }
    }

    if let Some(destination) = local_destination {
        if let Some(parent) = destination.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create local destination parent {}", parent.display()))?;
        }
        std::fs::write(&destination, received_data)
            .with_context(|| format!("write local destination {}", destination.display()))?;
        println!("copied {} -> {}", source, destination.display());
    } else {
        println!("copied {} -> remote:{}", source, destination);
    }

    let _ = client.close_file_stream(stream_id).await;
    let _ = server.close().await;
    let _ = client.close().await;
    Ok(())
}

fn shell_command_from_args(args: &[String]) -> Result<ShellCommand> {
    if args.is_empty() {
        #[cfg(windows)]
        {
            return Ok(ShellCommand::new("cmd").args([
                "/C",
                "echo",
                "blnk local fixture connected",
            ]));
        }
        #[cfg(not(windows))]
        {
            return Ok(ShellCommand::new("printf").arg("blnk local fixture connected\\n"));
        }
    }
    let program = args
        .first()
        .ok_or_else(|| anyhow!("shell command must not be empty"))?;
    Ok(ShellCommand::new(program).args(args.iter().skip(1).cloned()))
}

fn client_output(data: &[u8]) -> Result<()> {
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(data).context("write command output")?;
    stdout.flush().context("flush command output")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_fixture_command_is_non_empty() {
        let command = shell_command_from_args(&[]).expect("default command should build");
        assert!(!command.program().is_empty());
    }

    #[test]
    fn explicit_shell_command_preserves_argv_without_shell_interpolation() {
        let command = shell_command_from_args(&[
            "printf".to_owned(),
            "%s".to_owned(),
            "hello;not-a-shell-command".to_owned(),
        ])
        .expect("command should build");
        assert_eq!(command.program(), "printf");
        assert_eq!(command.arguments(), ["%s", "hello;not-a-shell-command"]);
    }

    #[test]
    fn unknown_device_is_rejected_before_remote_dial() {
        let registry = DeviceRegistry::default();
        let error = ensure_known_device(&registry, "missing-device")
            .expect_err("unknown device must be rejected");
        assert!(
            error
                .to_string()
                .contains("unknown device 'missing-device'")
        );
    }
}
