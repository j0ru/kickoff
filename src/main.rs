#![warn(clippy::nursery)]
#![allow(clippy::cast_possible_truncation)]

use anyhow::Result;
use app::App;
use clap::Parser;
use config::{Config, History};
use log::{debug, error, warn};
use std::time::Instant;
use std::{
    io::{Read, Write},
    {fs, io::ErrorKind},
    {path::PathBuf, process},
};
use xdg::BaseDirectories;

mod app;
mod color;
mod config;
mod font;
mod gui;
mod keybinds;
mod selection;

#[derive(Parser, Debug)]
pub struct Args {
    #[clap(short, long)]
    config: Option<PathBuf>,

    /// Set custom prompt, overwrites config if set
    #[clap(short, long)]
    prompt: Option<String>,

    /// Read list from stdin instead of PATH
    #[clap(long)]
    from_stdin: bool,

    /// Read list from PATH, default true, unless stdin is set
    #[clap(long)]
    from_path: bool,

    #[clap(long)]
    from_file: Vec<PathBuf>,

    /// Output selection to stdout instead of executing it
    #[clap(long)]
    stdout: bool,

    /// Set custom history name. Default history will only be used if stdin is not set
    #[clap(long)]
    history: Option<PathBuf>,
}

#[cfg(target_os = "linux")]
#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    match put_pid() {
        Ok(()) => {
            run().await?;
            del_pid()?;
            Ok(())
        }
        Err(e) => {
            error!("{e}");
            Ok(())
        }
    }
}

#[cfg(not(target_os = "linux"))]
#[tokio::main]
async fn main() -> Result<()> {
    run().await
}

#[cfg(target_os = "linux")]
fn put_pid() -> std::io::Result<()> {
    let xdg_dirs = BaseDirectories::with_prefix("kickoff");
    let pid_path = xdg_dirs.place_runtime_file("kickoff.pid").unwrap();
    match fs::File::open(pid_path.clone()) {
        Err(_) => {
            let mut pid_file = fs::File::create(pid_path)?;
            pid_file.write_all(std::process::id().to_string().as_bytes())?;
            Ok(())
        }
        Ok(mut file_handle) => {
            debug!("Pid file already exists");
            let mut pid = String::new();
            file_handle.read_to_string(&mut pid)?;
            if !pid.is_empty() && fs::metadata(format!("/proc/{pid}")).is_ok() {
                debug!("Pid from pid file still alive");
                Err(std::io::Error::new(
                    ErrorKind::Other,
                    "Kickoff is already running",
                ))
            } else {
                debug!("Pid from kickoff.pid not alive, overwriting...");
                let mut pid_file = fs::File::create(pid_path)?;
                pid_file.write_all(std::process::id().to_string().as_bytes())?;
                Ok(())
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn del_pid() -> std::io::Result<()> {
    let xdg_dirs = BaseDirectories::with_prefix("kickoff");
    let pid_path = xdg_dirs.place_runtime_file("kickoff.pid").unwrap();
    std::fs::remove_file(pid_path)?;
    Ok(())
}

async fn run() -> Result<()> {
    let start = Instant::now();
    let args = Args::parse();
    let config = match Config::load(args.config.clone()) {
        Ok(c) => c,
        Err(e) => {
            error!("{e}");
            process::exit(1);
        }
    };

    let history = if (!args.from_stdin && args.from_file.is_empty()) || args.history.is_some() {
        let path = args.history.clone();
        let decrease_interval = config.history.decrease_interval;
        Some(tokio::task::spawn_blocking(move || {
            History::load(path, decrease_interval)
        }))
    } else {
        None
    };

    let font = if let Some(font_name) = config.font.clone() {
        let mut font_names = config.fonts.clone();
        font_names.insert(0, font_name);
        font::Font::new(font_names, config.font_size)
    } else {
        font::Font::new(config.fonts.clone(), config.font_size)
    };

    let mut apps = selection::ElementListBuilder::new();
    if args.from_path || (!args.from_stdin && args.from_file.is_empty()) {
        apps.add_path(config.search.clone());
    }
    if !args.from_file.is_empty() {
        apps.add_files(&args.from_file);
    }
    if args.from_stdin {
        apps.add_stdin();
    }
    let apps = apps.build();
    let mut apps = apps.await?;

    let history = match history {
        Some(history) => {
            let history = history.await??;
            apps.merge_history(&history);
            Some(history)
        }
        None => None,
    };
    apps.sort_score();

    let elapsed = start.elapsed();
    debug!("Time till gui: {elapsed:?}");
    gui::run(App::new(args, config, apps, font.await?, history));

    Ok(())
}
