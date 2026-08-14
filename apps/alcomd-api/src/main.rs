use clap::Parser;

/// Optional loopback gateway for clients that cannot use native local IPC.
#[derive(Debug, Parser)]
#[command(name = "alcomd-api", version, about)]
struct Arguments {
    /// Loopback port. Zero means dynamically assigned.
    #[arg(long, default_value_t = 0)]
    port: u16,
}

fn main() {
    let arguments = Arguments::parse();
    eprintln!(
        "alcomd-api scaffold: gateway is not implemented; requested loopback port {}",
        arguments.port
    );
}
