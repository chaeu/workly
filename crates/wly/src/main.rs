use clap::Parser;

/// Workly CLI for coding agents.
#[derive(Parser)]
#[command(name = "wly", version = workly_core::VERSION)]
struct Cli {}

fn main() {
    Cli::parse();
}
