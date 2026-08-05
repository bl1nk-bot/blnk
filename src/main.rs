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
    Version,
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Serve(args) => run_serve(args),
        Commands::Connect(args) => run_connect(args),
        Commands::Cp(args) => run_cp(args),
        Commands::Version => print_version(),
    }
}