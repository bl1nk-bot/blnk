// src/main.rs
use clap::{Parser, Subcommand};

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Serve(ServeArgs),
    Connect(ConnectArgs),
    Cp(CpArgs),
    Devices(DevicesArgs),
    Version,
}

#[derive(Parser, Debug, Clone)]
struct ServeArgs {
    #[arg(long)]
    signaling_url: Option<String>,
}

#[derive(Parser, Debug, Clone)]
struct ConnectArgs {
    #[arg(long)]
    target: Option<String>,
}

#[derive(Parser, Debug, Clone)]
struct CpArgs {
    source: String,
    destination: String,
}

#[derive(Parser, Debug, Clone)]
struct DevicesArgs {
    #[arg(long)]
    list: bool,
}

fn run_serve(_args: ServeArgs) {
    println!("serve");
}

fn run_connect(_args: ConnectArgs) {
    println!("connect");
}

fn run_cp(_args: CpArgs) {
    println!("cp");
}

fn run_devices(_args: DevicesArgs) {
    println!("devices");
}

fn print_version() {
    println!("blnk 0.1.0");
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Serve(args) => run_serve(args),
        Commands::Connect(args) => run_connect(args),
        Commands::Cp(args) => run_cp(args),
        Commands::Devices(args) => run_devices(args),
        Commands::Version => print_version(),
    }
}
