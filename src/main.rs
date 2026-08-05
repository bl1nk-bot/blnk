// src/main.rs
use clap::{Parser, Subcommand};

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Serve,
    Connect,
    Cp,
    Version,
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Serve => println!("serve"),
        Commands::Connect => println!("connect"),
        Commands::Cp => println!("cp"),
        Commands::Version => println!("blnk 0.1.0"),
    }
}
