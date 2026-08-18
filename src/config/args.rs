use clap::Args;

#[derive(Debug, Args, Clone, Default)]
pub struct ServeArgs {
    /// Override the signaling server URL for this invocation.
    #[arg(long)]
    pub signaling_url: Option<String>,
}

#[derive(Debug, Args, Clone, Default)]
pub struct ConnectArgs {
    /// Peer or device target to connect to.
    #[arg(long)]
    pub target: Option<String>,
}

#[derive(Debug, Args, Clone)]
pub struct CpArgs {
    pub source: String,
    pub destination: String,
}

#[derive(Debug, Args, Clone, Default)]
pub struct DevicesArgs {
    /// List known devices.
    #[arg(long)]
    pub list: bool,
}
