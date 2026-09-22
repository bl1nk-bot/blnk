use clap::Args;
use std::net::IpAddr;

#[derive(Args, Clone, Default)]
pub struct ServeArgs {
    /// Override the signaling server URL for this invocation.
    #[arg(long)]
    pub signaling_url: Option<String>,
    /// Use the in-process loopback fixture instead of an external signaling server.
    #[arg(long)]
    pub local_fixture: bool,
    /// Stop after the fixture has started and printed its endpoint.
    #[arg(long)]
    pub once: bool,
    /// PIN used by the local fixture handshake.
    #[arg(long)]
    pub pin: Option<String>,
    /// Render terminal QR code for device pairing.
    #[arg(long)]
    pub qr: bool,
}

impl std::fmt::Debug for ServeArgs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServeArgs")
            .field("signaling_url", &self.signaling_url)
            .field("local_fixture", &self.local_fixture)
            .field("once", &self.once)
            .field("pin", &self.pin.as_ref().map(|_| "<redacted>"))
            .field("qr", &self.qr)
            .finish()
    }
}

#[derive(Args, Clone, Default)]
pub struct ConnectArgs {
    /// Peer or device target to connect to.
    #[arg(long)]
    pub target: Option<String>,
    /// Use the in-process loopback fixture instead of remote signaling/WebRTC orchestration.
    #[arg(long)]
    pub local_fixture: bool,
    /// Command and arguments to execute through the bounded shell stream in fixture mode.
    #[arg(long, num_args = 1..)]
    pub command: Vec<String>,
    /// PIN used by the local fixture handshake.
    #[arg(long)]
    pub pin: Option<String>,
}

impl std::fmt::Debug for ConnectArgs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectArgs")
            .field("target", &self.target)
            .field("local_fixture", &self.local_fixture)
            .field("command", &self.command)
            .field("pin", &self.pin.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

#[derive(Args, Clone)]
pub struct CpArgs {
    /// Peer or device target for remote signaling/file transfer.
    #[arg(long)]
    pub target: Option<String>,
    pub source: String,
    pub destination: String,
    /// Use the in-process loopback fixture for a real authenticated file transfer.
    #[arg(long)]
    pub local_fixture: bool,
    /// Replace an existing destination in the receiver sandbox.
    #[arg(long)]
    pub overwrite: bool,
    /// PIN used by the remote authenticated session.
    #[arg(long)]
    pub pin: Option<String>,
}

impl std::fmt::Debug for CpArgs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CpArgs")
            .field("target", &self.target)
            .field("source", &self.source)
            .field("destination", &self.destination)
            .field("local_fixture", &self.local_fixture)
            .field("overwrite", &self.overwrite)
            .field("pin", &self.pin.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

#[derive(Args, Clone)]
pub struct WebArgs {
    /// Bind address; the browser control surface accepts loopback addresses only.
    #[arg(long, default_value = "127.0.0.1", value_parser = clap::value_parser!(IpAddr))]
    pub host: IpAddr,
    /// TCP port, or 0 to let the operating system select an ephemeral port.
    #[arg(long, default_value_t = 0)]
    pub port: u16,
    /// Exact browser Origin allowed by the API and WebSocket handshake.
    #[arg(long, default_value = "http://127.0.0.1:3000")]
    pub origin: String,
    /// Bootstrap bearer token; can also be provided through BLNK_WEB_TOKEN.
    #[arg(long)]
    pub bootstrap_token: Option<String>,
}

impl std::fmt::Debug for WebArgs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebArgs")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("origin", &self.origin)
            .field("bootstrap_token", &self.bootstrap_token.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

#[derive(Debug, Args, Clone, Default)]
pub struct DevicesArgs {
    /// List known devices.
    #[arg(long)]
    pub list: bool,
    /// Discover peers on local LAN via mDNS.
    #[arg(long)]
    pub local: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Debug, Parser)]
    struct TestCli {
        #[command(subcommand)]
        command: TestCommand,
    }

    #[derive(Debug, clap::Subcommand)]
    enum TestCommand {
        Serve(ServeArgs),
        Connect(ConnectArgs),
        Cp(CpArgs),
        Devices(DevicesArgs),
        Web(WebArgs),
    }

    #[test]
    fn remote_cp_target_pin_and_overwrite_are_parseable() {
        let cli = TestCli::try_parse_from([
            "blnk",
            "cp",
            "--target",
            "device-1",
            "--pin",
            "123456",
            "--overwrite",
            "source.txt",
            "dest.txt",
        ])
        .expect("remote cp args should parse");
        let TestCommand::Cp(args) = cli.command else {
            panic!("expected cp command");
        };
        assert_eq!(args.target.as_deref(), Some("device-1"));
        assert_eq!(args.pin.as_deref(), Some("123456"));
        assert!(args.overwrite);
        assert_eq!(args.source, "source.txt");
        assert_eq!(args.destination, "dest.txt");
    }

    #[test]
    fn web_loopback_options_are_parseable() {
        let cli = TestCli::try_parse_from([
            "blnk",
            "web",
            "--host",
            "127.0.0.1",
            "--port",
            "8080",
            "--origin",
            "http://127.0.0.1:3000",
            "--bootstrap-token",
            "fixture-token",
        ])
        .expect("web args should parse");
        let TestCommand::Web(args) = cli.command else {
            panic!("expected web command");
        };
        assert_eq!(
            args.host,
            "127.0.0.1".parse::<IpAddr>().expect("loopback ip")
        );
        assert_eq!(args.port, 8080);
        assert_eq!(args.origin, "http://127.0.0.1:3000");
        assert_eq!(args.bootstrap_token.as_deref(), Some("fixture-token"));
    }

    #[test]
    fn fixture_flags_and_command_are_parseable() {
        let cli = TestCli::try_parse_from([
            "blnk",
            "connect",
            "--local-fixture",
            "--command",
            "printf",
            "hello",
        ])
        .expect("fixture connect args should parse");
        let TestCommand::Connect(args) = cli.command else {
            panic!("expected connect command");
        };
        assert!(args.local_fixture);
        assert_eq!(args.command, ["printf", "hello"]);
    }

    #[test]
    fn serve_and_devices_flags_are_parseable() {
        let cli = TestCli::try_parse_from([
            "blnk",
            "serve",
            "--signaling-url",
            "ws://127.0.0.1:9000",
            "--local-fixture",
            "--once",
            "--pin",
            "654321",
            "--qr",
        ])
        .expect("serve args should parse");
        let TestCommand::Serve(args) = cli.command else {
            panic!("expected serve command");
        };
        assert_eq!(args.signaling_url.as_deref(), Some("ws://127.0.0.1:9000"));
        assert!(args.local_fixture);
        assert!(args.once);
        assert_eq!(args.pin.as_deref(), Some("654321"));
        assert!(args.qr);

        let cli_devices = TestCli::try_parse_from(["blnk", "devices", "--list", "--local"])
            .expect("devices args should parse");
        let TestCommand::Devices(dev_args) = cli_devices.command else {
            panic!("expected devices command");
        };
        assert!(dev_args.list);
        assert!(dev_args.local);
    }

    #[test]
    fn args_debug_redacts_secrets() {
        let serve = ServeArgs {
            pin: Some("secret123".to_owned()),
            ..ServeArgs::default()
        };
        let serve_debug = format!("{serve:?}");
        assert!(!serve_debug.contains("secret123"));
        assert!(serve_debug.contains("<redacted>"));

        let connect = ConnectArgs {
            pin: Some("secret456".to_owned()),
            ..ConnectArgs::default()
        };
        let connect_debug = format!("{connect:?}");
        assert!(!connect_debug.contains("secret456"));
        assert!(connect_debug.contains("<redacted>"));

        let cp = CpArgs {
            target: None,
            source: "a".into(),
            destination: "b".into(),
            local_fixture: false,
            overwrite: false,
            pin: Some("secret789".to_owned()),
        };
        let cp_debug = format!("{cp:?}");
        assert!(!cp_debug.contains("secret789"));
        assert!(cp_debug.contains("<redacted>"));

        let web = WebArgs {
            host: "127.0.0.1".parse().expect("ip"),
            port: 0,
            origin: "http://127.0.0.1:3000".into(),
            bootstrap_token: Some("secret_token".to_owned()),
        };
        let web_debug = format!("{web:?}");
        assert!(!web_debug.contains("secret_token"));
        assert!(web_debug.contains("<redacted>"));
    }
}
