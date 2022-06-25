use crate::config::Config;
use crate::gui::{Action, DData, RenderEvent};
use clap::Parser;
use history::History;
use image::ImageBuffer;
use log::*;
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
use std::{cmp, error::Error, path::PathBuf, process, time::Duration};
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
    #[clap(short, long)]
    config: Option<PathBuf>,

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

    let mut apps = selection::ElementList::new();
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

    let history = if (!args.from_stdin && args.from_file.is_empty()) || args.history.is_some() {
        let path = args.history.clone();
        let decrease_interval = config.history.decrease_interval;
        Some(tokio::task::spawn_blocking(move || {
            History::load(path, decrease_interval)
        }))
    } else {
        None
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

    let mut search_results = apps.as_ref_vec();
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
                    if select_query && !search_results.is_empty() {
                        select_query = false;
                    } else if !search_results.is_empty() && selection < search_results.len() - 1 {
                        selection += 1;
                    }
                }
                Action::Search => {
                    need_redraw = true;
                    search_results = apps.search(query);
                    select_query = false;
                    selection = 0;
                    if search_results.is_empty() {
                        select_query = true
                    }
                }
                Action::Complete => {
                    if !select_query {
                        let app = search_results.get(selection).unwrap();
                        if query == &app.name {
                            selection = if selection < search_results.len() - 1 {
                                selection + 1
                            } else {
                                selection
                            };
                        }
                        query.clear();
                        query.push_str(&search_results.get(selection).unwrap().name);
                        need_redraw = true;
                    }
                }
                Action::Execute => {
                    let element = if select_query {
                        selection::Element {
                            name: query.to_string(),
                            value: query.to_string(),
                            base_score: 0,
                        }
                    } else {
                        (*search_results.get(selection).unwrap()).clone()
                    };
                    if args.stdout {
                        print!("{}", element.value);
                        if let Some(mut history) = history {
                            history.inc(&element.value);
                            history.save()?;
                        }
                        return Ok(None);
                    } else {
                        return Ok(Some(exec(element, history)?));
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

            for (i, matched) in search_results
                .iter()
                .enumerate()
                .take(cmp::min(max_entries + offset, search_results.len()))
                .skip(offset)
            {
                let color = if i == selection && !select_query {
                    &config.colors.text_selected
                } else {
                    &config.colors.text
                };
                font.render(
                    &matched.name,
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

fn exec(
    elem: selection::Element,
    history: Option<History>,
) -> Result<tokio::task::JoinHandle<()>, Box<dyn Error>> {
    let command = elem.value;
    match unsafe { fork() } {
        Ok(ForkResult::Parent { child }) => {
            Ok(tokio::spawn(async move {
                tokio::time::sleep(Duration::new(1, 0)).await;
                match waitpid(child, Some(WaitPidFlag::WNOHANG)) {
                    Ok(WaitStatus::StillAlive) | Ok(WaitStatus::Exited(_, 0)) => {
                        if let Some(mut history) = history {
                            history.inc(&command);
                            match history.save() {
                                Ok(()) => {}
                                Err(e) => {
                                    error!("{}", e);
                                }
                            };
                        }
                    }
                    Ok(_) => {
                        /* Every non 0 statuscode holds no information since it's
                        origin can be the started application or a file not found error.
                        In either case the error has already been logged and does not
                        need to be handled here. */
                    }
                    Err(err) => error!("{}", err),
                }
            }))
        }
        Ok(ForkResult::Child) => {
            let err = exec::Command::new("sh").args(&["-c", &command]).exec();

            // Won't be executed when exec was successful
            error!("{}", err);

            Notification::new()
                .summary("Kickoff")
                .body(&format!("{}", err))
                .timeout(5000)
                .show()?;
            process::exit(2);
        }
        Err(e) => Err(Box::new(e)),
    }
}
