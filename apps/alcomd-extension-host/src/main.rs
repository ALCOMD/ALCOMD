use clap::Parser;

/// Sandboxed host for first-party and third-party background extensions.
#[derive(Debug, Parser)]
#[command(name = "alcomd-extension-host", version, about)]
struct Arguments {
    /// Extension identifier to host.
    #[arg(long)]
    extension: Option<String>,
}

fn main() {
    let arguments = Arguments::parse();
    match arguments.extension {
        Some(extension) => {
            eprintln!("extension host scaffold: {extension} was not started");
        }
        None => {
            eprintln!("extension host scaffold: no extension selected");
        }
    }
}
