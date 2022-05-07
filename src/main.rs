use crate::config::Config;
use crate::gui::{Action, DData, RenderEvent};
use clap::Parser;
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use image::ImageBuffer;
use log::error;
use nix::{
    sys::wait::{waitpid, WaitPidFlag, WaitStatus},
    unistd::{fork, ForkResult},
};
use notify_rust::Notification;
use smithay_client_toolkit::{
    default_environment,
    environment::SimpleGlobal,
    new_default_environment,
    reexports::{calloop, protocols::wlr::unstable::layer_shell::v1::client::zwlr_layer_shell_v1},
    WaylandSource,
};
use std::{cmp, collections::HashMap, error::Error, path::PathBuf, process, time::Duration};
use tokio::task::JoinHandle;

mod color;
mod config;
mod font;
mod gui;
mod history;
mod keybinds;
mod selection;

default_environment!(Env,
    fields = [
        layer_shell: SimpleGlobal<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
    ],
    singles = [
        zwlr_layer_shell_v1::ZwlrLayerShellV1 => layer_shell
    ],
);

#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {
    /// Use config at custom path
    #[clap(short, long)]
    config: Option<PathBuf>,

    /// Read list from stdin instead of PATH
    #[clap(long)]
    stdin: bool,

    /// Output selection to stdout instead of executing it
    #[clap(long)]
    stdout: bool,

    /// Set custom history name. Default history will only be used if stdin is not set
    #[clap(long)]
    history: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    if let Some(child_handle) = run().await? {
        /* wait for check if comand exec was successful
           and history has been written
        */
        child_handle.await?;
    }
    Ok(())
}

async fn run() -> Result<Option<JoinHandle<()>>, Box<dyn Error>> {
    env_logger::init();

    let args = Args::parse();

    let config = match Config::load(args.config) {
        Ok(c) => c,
        Err(e) => {
            error!("{}", e);
            process::exit(1);
        }
    };

    let mut elems = if args.stdin {
        selection::ElementList::from_stdin().await?
    } else {
        selection::ElementList::from_path().await?
    };

    let font = if let Some(font_name) = config.font {
        let mut font_names = config.fonts.clone();
        font_names.insert(0, font_name);
        font::Font::new(font_names, config.font_size)
    } else {
        font::Font::new(config.fonts, config.font_size)
    };

    let (env, display, queue) =
        new_default_environment!(Env, fields = [layer_shell: SimpleGlobal::new(),])
            .expect("Initial roundtrip failed!");

    let mut history_file = history::History::load(None).await?;
    elems.merge_history(&history_file);
    elems.sort();

    let history = history_file.as_hashmap();
    let applications = elems.as_value_list();

    let layer_shell = env.require_global::<zwlr_layer_shell_v1::ZwlrLayerShellV1>();
    let pools = env
        .create_double_pool(|_| {})
        .expect("Failed to create a memory pool!");
    let surface = env.create_surface().detach();
    let mut surface = gui::Surface::new(None, surface, &layer_shell, pools);

    let mut event_loop = calloop::EventLoop::<DData>::try_new().unwrap();
    WaylandSource::new(queue)
        .quick_insert(event_loop.handle())
        .unwrap();

    gui::register_inputs(&env.get_all_seats(), &event_loop);

    let mut matched_exe: Vec<&String> = applications.iter().collect();
    let mut need_redraw = false;
    let mut data = DData::new(&display, config.keybindings.clone().into());
    let mut selection = 0;
    let mut select_query = false;
    let mut font = font.await?;

    loop {
        let gui::DData { query, action, .. } = &mut data;
        match surface.next_render_event.take() {
            Some(RenderEvent::Closed) => break,
            Some(RenderEvent::Configure { width, height }) => {
                need_redraw = surface.set_dimensions(width, height);
            }
            None => {}
        }
        if let Some(action) = action.take() {
            match action {
                Action::NavUp => {
                    need_redraw = true;
                    if selection > 0 {
                        selection -= 1;
                    } else if !query.is_empty() {
                        select_query = true;
                    }
                }
                Action::NavDown => {
                    need_redraw = true;
                    if select_query && !matched_exe.is_empty() {
                        select_query = false;
                    } else if !matched_exe.is_empty() && selection < matched_exe.len() - 1 {
                        selection += 1;
                    }
                }
                Action::Search => {
                    need_redraw = true;
                    matched_exe = fuzzy_sort(&applications, query, &history);
                    select_query = false;
                    selection = 0;
                    if matched_exe.is_empty() {
                        select_query = true
                    }
                }
                Action::Complete => {
                    if !select_query {
                        let app = matched_exe.get(selection).unwrap();
                        if query == *app {
                            selection = if selection < matched_exe.len() - 1 {
                                selection + 1
                            } else {
                                selection
                            };
                        }
                        query.clear();
                        query.push_str(matched_exe.get(selection).unwrap());
                        need_redraw = true;
                    }
                }
                Action::Execute => {
                    let query = if select_query {
                        query.to_string()
                    } else {
                        matched_exe.get(selection).unwrap().to_string()
                    };
                    if let Ok(mut args) = shellwords::split(&query) {
                        match unsafe { fork() } {
                            Ok(ForkResult::Parent { child }) => {
                                return Ok(Some(tokio::spawn(async move {
                                    tokio::time::sleep(Duration::new(1, 0)).await;
                                    match waitpid(child, Some(WaitPidFlag::WNOHANG)) {
                                        Ok(WaitStatus::StillAlive)
                                        | Ok(WaitStatus::Exited(_, 0)) => {
                                            history_file.inc(&query);
                                            match history_file.save(None).await {
                                                Ok(()) => {}
                                                Err(e) => {
                                                    error!("{}", e);
                                                }
                                            };
                                        }
                                        Ok(_) => {
                                            /* Every non 0 statuscode holds no information since it's
                                            origin can be the started application or a file not found error.
                                            In either case the error has already been logged and does not
                                            need to be handled here. */
                                        }
                                        Err(err) => error!("{}", err),
                                    }
                                })));
                            }
                            Ok(ForkResult::Child) => {
                                let err = exec::Command::new(args.remove(0)).args(&args).exec();

                                // Won't be executed when exec was successful
                                error!("{}", err);

                                Notification::new()
                                    .summary("Kickoff")
                                    .body(&format!("{}", err))
                                    .timeout(5000)
                                    .show()?;
                                process::exit(2);
                            }
                            Err(err) => {
                                error!("{}", err);
                            }
                        }
                        break;
                    }
                }
                Action::Exit => break,
                _ => {}
            }
        }

        if need_redraw {
            need_redraw = false;

            let mut img = ImageBuffer::from_pixel(
                surface.dimensions.0,
                surface.dimensions.1,
                config.colors.background.to_rgba(),
            );
            let prompt_width = if !config.prompt.is_empty() {
                let (width, _) = font.render(
                    &config.prompt,
                    &config.colors.prompt,
                    &mut img,
                    config.padding,
                    config.padding,
                );
                width
            } else {
                0
            };

            if !query.is_empty() {
                let color = if select_query {
                    &config.colors.text_selected
                } else {
                    &config.colors.text_query
                };
                font.render(
                    query,
                    color,
                    &mut img,
                    config.padding + prompt_width,
                    config.padding,
                );
            }

            let spacer = (1.5 * config.font_size) as u32;
            let max_entries = ((surface.dimensions.1 - 2 * config.padding - spacer) as f32
                / (config.font_size * 1.2)) as usize;
            let offset = if selection > (max_entries / 2) {
                (selection - max_entries / 2) as usize
            } else {
                0
            };

            for (i, matched) in matched_exe
                .iter()
                .enumerate()
                .take(cmp::min(max_entries + offset, matched_exe.len()))
                .skip(offset)
            {
                let color = if i == selection && !select_query {
                    &config.colors.text_selected
                } else {
                    &config.colors.text
                };
                font.render(
                    matched,
                    color,
                    &mut img,
                    config.padding,
                    (config.padding
                        + spacer
                        + (i - offset) as u32 * (config.font_size * 1.2) as u32)
                        as u32,
                );
            }

            match surface.draw(&img) {
                Ok(_) => {}
                Err(e) => {
                    error!("{}", e);
                    need_redraw = false;
                }
            };
        }

        display.flush().unwrap();
        event_loop.dispatch(None, &mut data).unwrap();
    }
    Ok(None)
}

fn fuzzy_sort<'a>(
    executables: &'a [String],
    pattern: &str,
    pre_scored: &'a HashMap<String, usize>,
) -> Vec<&'a String> {
    let matcher = SkimMatcherV2::default();
    let mut executables = executables
        .iter()
        .map(|x| {
            (
                matcher
                    .fuzzy_match(x, pattern)
                    .map(|score| score + *pre_scored.get(x).unwrap_or(&1) as i64),
                x,
            )
        })
        .filter(|x| x.0.is_some())
        .collect::<Vec<(Option<i64>, &String)>>();
    executables.sort_by(|a, b| b.0.unwrap_or(0).cmp(&a.0.unwrap_or(0)));
    executables.into_iter().map(|x| x.1).collect()
}
