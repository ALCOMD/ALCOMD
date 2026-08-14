use clap::Parser;

/// One-time v3 migration executable.
///
/// This scaffold intentionally contains no legacy readers.
#[derive(Debug, Parser)]
#[command(name = "alcomd-migrate-v3", version, about)]
struct Arguments {
    /// Validate the migration bundle without writing v4 state.
    #[arg(long)]
    dry_run: bool,
}

fn main() {
    let arguments = Arguments::parse();
    if arguments.dry_run {
        println!("migration scaffold dry-run: no legacy readers are implemented");
    } else {
        eprintln!("migration scaffold refused to run without an audited migration plan");
        std::process::exit(2);
    }
}
