mod config;
mod cover;
mod icons;
mod jukebox;
mod spotify;
mod theme;

use jukebox::{Jukebox, Startup};
use std::{
    path::{Path, PathBuf},
    process::ExitCode,
};

const COMMAND: &str = env!("CARGO_BIN_NAME");
const HELP: &str = "\
Play Spotify in your terminal.

Usage:
  spt [--config FILE]        open the player (logs in through your browser the first time)
  spt check-config [FILE]    check a settings file, ~/.config/spt/config.toml by default
  spt devices                list the audio outputs playback.output can name
  spt logout                 remove your Spotify login from this computer
  spt --version              print the version

Playback needs Spotify Premium. Press ? in the player for keys.";
/// How long the player may take to close after it is told to, before it is stopped.
const SHUTDOWN_GRACE: std::time::Duration = std::time::Duration::from_secs(3);

fn main() -> ExitCode {
    // Returning the error from `main` would print its `Debug` form instead of its message.
    match dispatch() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{COMMAND}: {error}");
            ExitCode::FAILURE
        }
    }
}

fn dispatch() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments: Vec<String> = std::env::args().skip(1).collect();
    let config_path = match arguments.iter().position(|argument| argument == "--config") {
        Some(index) if index + 1 < arguments.len() => {
            let path = arguments.remove(index + 1);
            arguments.remove(index);
            Some(PathBuf::from(path))
        }
        Some(_) => return Err("--config needs a file".into()),
        None => None,
    };
    match arguments
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        [] => run(config_path.as_deref()),
        ["check-config"] => check_config(config_path.as_deref()),
        ["check-config", path] => check_config(Some(Path::new(path))),
        ["devices"] => {
            for name in spotify::output_names()? {
                println!("{name}");
            }
            Ok(())
        }
        ["logout"] => {
            spotify::forget()?;
            println!(
                "Logged out. Your Spotify login and Client ID are removed from this computer."
            );
            Ok(())
        }
        ["--version" | "-V"] => {
            println!("{COMMAND} {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        ["--help" | "-h"] => {
            println!("{HELP}");
            Ok(())
        }
        _ => Err(format!("Unknown arguments; run {COMMAND} --help for usage").into()),
    }
}

fn check_config(path: Option<&Path>) -> Result<(), Box<dyn std::error::Error>> {
    config::load(path)?;
    let shown = path.map(Path::to_path_buf).or_else(config::default_path);
    println!(
        "{}: valid",
        shown.map_or_else(|| "the defaults".into(), |path| path.display().to_string())
    );
    Ok(())
}

fn run(config_path: Option<&Path>) -> Result<(), Box<dyn std::error::Error>> {
    // Before the terminal changes, so that a mistake in the settings prints plainly.
    let config = config::load(config_path)?;
    spotify::check_output(&config.playback.output)?;
    config::install(config);
    let config = config::get();
    stop_if_stuck()?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let _runtime = runtime.enter();
    let startup = Startup {
        http: reqwest::Client::new(),
        refresh_token: spotify::stored_refresh_token()?,
    };
    let options = hypercmd::native::NativeOptions {
        layout: hypercmd::layout::LayoutOptions {
            overrides: theme::stylesheet(config),
            ..Default::default()
        },
        ..Default::default()
    };
    hypercmd::native::run_with(hypercmd::mount::<Jukebox>(startup)?, &options)?;
    if cover::supported() {
        // kitty keeps an image until told otherwise, even once the player is gone.
        cover::clear(cover::Slot::Player)?;
        cover::clear(cover::Slot::Header)?;
    }
    Ok(())
}

/// Ends the process if it is still there a moment after a hangup, Ctrl+C or `q`.
/// Closing the terminal while music played once left the player playing on, out
/// of sight, with every later signal swallowed by the terminal runner.
fn stop_if_stuck() -> std::io::Result<()> {
    use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM};
    let mut signals = signal_hook::iterator::Signals::new([SIGHUP, SIGINT, SIGTERM])?;
    std::thread::spawn(move || {
        if signals.forever().next().is_some() {
            std::thread::sleep(SHUTDOWN_GRACE);
            std::process::exit(1);
        }
    });
    Ok(())
}
