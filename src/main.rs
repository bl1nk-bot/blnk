use anyhow::{Context, Result, anyhow, bail};
use blnk::config::args::{ConnectArgs, CpArgs, DevicesArgs, ServeArgs, WebArgs};
use blnk::config::{Config, DeviceRegistry};
use blnk::identity::Identity;
use blnk::peer::TwoPeerHarness;
use blnk::session::{SessionRuntime, SessionRuntimeConfig};
use blnk::signaling::EndpointPolicy;
use blnk::signaling::orchestration::{
    AuditLog, DEFAULT_ORCHESTRATION_TIMEOUT, OperationScope, accept_server_session, connect_target,
    run_file_client, run_shell_client, serve_session_with_shutdown,
};
use blnk::stream::shell::ShellCommand;
use blnk::web::{BrowserControlConfig, BrowserControlServer};
use clap::{Parser, Subcommand};
use std::net::SocketAddr;
use std::path::Path;
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
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();

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

    let mdns_responder = blnk::discovery::MdnsResponder::new();
    let _ = mdns_responder.start_announcing(identity.uid(), 0);

    if args.qr {
        let pairing_payload = blnk::utils::qr::PairingQrPayload {
            uid: identity.uid().to_string(),
            pairing_code: identity.pairing_code().to_string(),
            endpoint: signaling_url.clone(),
        };
        let payload_str = pairing_payload.to_payload_string();
        println!("\n--- Pairing QR Code ---");
        match blnk::utils::qr::render_qr_terminal(&payload_str)
            .or_else(|_| blnk::utils::qr::render_qr_ascii(&payload_str))
        {
            Ok(qr_rendered) => println!("{qr_rendered}"),
            Err(err) => println!("Failed to render QR: {err}"),
        }
        println!("-----------------------\n");
    }

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
    let session_task = serve_session_with_shutdown(
        session,
        root,
        shutdown.clone(),
        OperationScope::default(),
        AuditLog::new(""),
    );
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
    if args.local {
        println!("Discovering blnk peers on local network (mDNS)...");
        let peers =
            blnk::discovery::discover_local_peers(blnk::discovery::DEFAULT_MDNS_DISCOVERY_TIMEOUT)
                .await
                .context("mDNS peer discovery failed")?;

        if peers.is_empty() {
            println!("No local blnk peers found.");
            return Ok(());
        }

        println!("ID\tIP\tPORT\tHOST");
        for peer in peers {
            println!(
                "{}\t{}\t{}\t{}",
                peer.id,
                peer.ip,
                peer.port,
                peer.host_name.as_deref().unwrap_or("-")
            );
        }
        return Ok(());
    }

    let config = Config::load().context("load blnk configuration")?;
    let registry = DeviceRegistry::load(&config.devices_path).context("load device registry")?;
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
    let (mut client, server) = fixture_pair(pin).await?;
    let root = std::env::current_dir().context("get shell fixture root")?;
    let server_task = tokio::spawn(serve_session_with_shutdown(
        blnk::signaling::orchestration::ServerSession {
            runtime: server,
            client_id: LOCAL_FIXTURE_DEVICE_ID.to_string(),
        },
        root,
        CancellationToken::new(),
        OperationScope::default(),
        AuditLog::new(LOCAL_FIXTURE_DEVICE_ID),
    ));

    let result = run_shell_client(&mut client, &command).await;
    let close_result = client.close().await;
    let _ = server_task.await;

    match (result, close_result) {
        (Err(error), _) => Err(error.into()),
        (Ok(_), Err(error)) => Err(error.into()),
        (Ok(code), Ok(())) => Ok(code),
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
    let (mut client, server) = fixture_pair(DEFAULT_FIXTURE_PIN).await?;
    let server_task = tokio::spawn(serve_session_with_shutdown(
        blnk::signaling::orchestration::ServerSession {
            runtime: server,
            client_id: LOCAL_FIXTURE_DEVICE_ID.to_string(),
        },
        fixture_root.to_path_buf(),
        CancellationToken::new(),
        OperationScope::default(),
        AuditLog::new(LOCAL_FIXTURE_DEVICE_ID),
    ));

    let result = run_file_client(&mut client, source, destination, overwrite).await;
    let close_result = client.close().await;
    let _ = server_task.await;

    match (result, close_result) {
        (Err(error), _) => Err(error.into()),
        (Ok(_), Err(error)) => Err(error.into()),
        (Ok(()), Ok(())) => Ok(()),
    }
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
