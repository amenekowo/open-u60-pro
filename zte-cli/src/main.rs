mod cmd;
mod ui;

use clap::Parser;

#[derive(Parser)]
#[command(name = "zte", version, about = "ZTE U60 Pro Toolkit")]
struct Cli {
    #[command(subcommand)]
    command: cmd::Commands,
}

fn main() {
    let cli = Cli::parse();
    if let Err(e) = cmd::run(cli.command) {
        eprintln!("\x1b[31mError:\x1b[0m {e}");
        std::process::exit(1);
    }
}
