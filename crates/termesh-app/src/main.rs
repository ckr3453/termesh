mod app;
mod cli;

use app::App;
use clap::Parser;
use cli::{Cli, Command};
use termesh_platform::event_loop::{self, PlatformConfig};

fn main() {
    let app = match Cli::parse().command {
        Some(Command::Open { name }) => match App::open_workspace(&name) {
            Ok(app) => app,
            Err(e) => {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        },
        None => App::new(),
    };

    let config = PlatformConfig {
        font_size: 14.0,
        scrollback: 10_000,
        input_handler: app.input().clone(),
    };

    if let Err(e) = event_loop::run(config) {
        eprintln!("Fatal: {e}");
        std::process::exit(1);
    }
}
