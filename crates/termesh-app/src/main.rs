mod app;
mod cli;

use app::App;
use clap::Parser;
use cli::{Cli, Command};

fn main() {
    let cli = Cli::parse();

    let app = match cli.command {
        Some(Command::Open { name }) => match App::open_workspace(&name) {
            Ok(app) => app,
            Err(e) => {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        },
        None => App::new(),
    };

    // Phase 1: print startup info and exit
    // Full event loop integration will connect platform + renderer + pty
    println!(
        "termesh v{} - {:?} mode, {} session(s)",
        env!("CARGO_PKG_VERSION"),
        app.view_mode(),
        app.focus_layout().sessions().len(),
    );
}
