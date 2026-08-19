use anyhow::Result;
use blnk::config::args::{ConnectArgs, CpArgs, DevicesArgs, ServeArgs};
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "blnk", version, about = "Remote access multitool")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Start the local service (implementation pending).
    Serve(ServeArgs),
    /// Connect to a peer or device (implementation pending).
    Connect(ConnectArgs),
    /// Copy data through a session (implementation pending).
    Cp(CpArgs),
    /// List known devices (implementation pending).
    Devices(DevicesArgs),
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
        Commands::Version => {
            println!("blnk {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
    }
}

async fn run_serve(_args: ServeArgs) -> Result<()> {
    println!("serve command is not implemented yet");
    Ok(())
}

async fn run_connect(_args: ConnectArgs) -> Result<()> {
    println!("connect command is not implemented yet");
    Ok(())
}

async fn run_cp(_args: CpArgs) -> Result<()> {
    println!("cp command is not implemented yet");
    Ok(())
}

async fn run_devices(_args: DevicesArgs) -> Result<()> {
    println!("devices command is not implemented yet");
    Ok(())
}
