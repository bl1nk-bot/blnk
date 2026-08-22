use clap::Args;

#[derive(Debug, Args, Clone, Default)]
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
}

#[derive(Debug, Args, Clone, Default)]
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

#[derive(Debug, Args, Clone)]
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

#[derive(Debug, Args, Clone, Default)]
pub struct DevicesArgs {
    /// List known devices.
    #[arg(long)]
    pub list: bool,
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
}
