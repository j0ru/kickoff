use std::{path::PathBuf, process};

use anyhow::Result;
use app::App;
use clap::Parser;
use config::{Config, History};
use log::*;

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

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
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
        apps.add_path();
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

    gui::run(App::new(args, config, apps, font.await?, history));

    info!("quitting kickoff");

    Ok(())
}
