#![feature(test)]
use smithay_client_toolkit::{
    default_environment,
    environment::SimpleGlobal,
    new_default_environment,
    reexports::{
        calloop,
        client::protocol::{
            wl_keyboard, wl_output,
            wl_pointer::{ButtonState, Event as PEvent},
            wl_shm, wl_surface,
        },
        client::{Attached, DispatchData, Main},
        protocols::wlr::unstable::layer_shell::v1::client::{
            zwlr_layer_shell_v1, zwlr_layer_surface_v1,
        },
    },
    seat::{
        keyboard::{
            keysyms, map_keyboard_repeat, Event as KbEvent, KeyState, ModifiersState, RepeatKind,
        },
        with_seat_data,
    },
    shm::DoubleMemPool,
    WaylandSource,
};

use smithay_clipboard::Clipboard;

use std::cell::Cell;
use std::io::{BufWriter, ErrorKind, Seek, SeekFrom, Write};
use std::rc::Rc;

use image::{ImageBuffer, RgbaImage};

use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use std::collections::HashMap;
use std::{cmp, env, fs, process};

use futures::executor::block_on;
use futures::join;
use nix::unistd::{fork, ForkResult};

mod color;
mod config;
mod font;
mod history;

default_environment!(Env,
    fields = [
        layer_shell: SimpleGlobal<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
    ],
    singles = [
        zwlr_layer_shell_v1::ZwlrLayerShellV1 => layer_shell
    ],
);

#[derive(PartialEq, Copy, Clone)]
enum RenderEvent {
    Configure { width: u32, height: u32 },
    Closed,
}

struct Surface {
    surface: wl_surface::WlSurface,
    layer_surface: Main<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1>,
    next_render_event: Rc<Cell<Option<RenderEvent>>>,
    pools: DoubleMemPool,
    dimensions: (u32, u32),
}

impl Surface {
    fn new(
        output: Option<&wl_output::WlOutput>,
        surface: wl_surface::WlSurface,
        layer_shell: &Attached<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
        pools: DoubleMemPool,
    ) -> Self {
        let layer_surface = layer_shell.get_layer_surface(
            &surface,
            output,
            zwlr_layer_shell_v1::Layer::Overlay,
            "launcher".to_owned(),
        );

        // Anchor to the top left corner of the output
        layer_surface.set_anchor(zwlr_layer_surface_v1::Anchor::all());

        // Enable Keyboard interactivity
        layer_surface.set_keyboard_interactivity(1);

        let next_render_event = Rc::new(Cell::new(None::<RenderEvent>));
        let next_render_event_handle = Rc::clone(&next_render_event);
        layer_surface.quick_assign(move |layer_surface, event, _| {
            match (event, next_render_event_handle.get()) {
                (zwlr_layer_surface_v1::Event::Closed, _) => {
                    next_render_event_handle.set(Some(RenderEvent::Closed));
                }
                (
                    zwlr_layer_surface_v1::Event::Configure {
                        serial,
                        width,
                        height,
                    },
                    next,
                ) if next != Some(RenderEvent::Closed) => {
                    layer_surface.ack_configure(serial);
                    next_render_event_handle.set(Some(RenderEvent::Configure { width, height }));
                }
                (_, _) => {}
            }
        });

        // Commit so that the server will send a configure event
        surface.commit();

        Self {
            surface,
            layer_surface,
            next_render_event,
            pools,
            dimensions: (0, 0),
        }
    }

    fn draw(&mut self, image: &RgbaImage) -> Result<(), std::io::Error> {
        if let Some(pool) = self.pools.pool() {
            let stride = 4 * self.dimensions.0 as i32;
            let width = self.dimensions.0 as i32;
            let height = self.dimensions.1 as i32;

            // First make sure the pool is the right size
            pool.resize((stride * height) as usize)?;

            // Create a new buffer from the pool
            let buffer = pool.buffer(0, width, height, stride, wl_shm::Format::Abgr8888);

            // Write the color to all bytes of the pool
            pool.seek(SeekFrom::Start(0))?;
            {
                let mut writer = BufWriter::new(&mut *pool);
                writer.write_all(image.as_raw())?;
                writer.flush()?;
            }

            // Attach the buffer to the surface and mark the entire surface as damaged
            self.surface.attach(Some(&buffer), 0, 0);
            self.surface
                .damage_buffer(0, 0, width as i32, height as i32);

            // Finally, commit the surface
            self.surface.commit();
            Ok(())
        } else {
            Err(std::io::Error::new(
                ErrorKind::Other,
                "All pools are in use by Wayland",
            ))
        }
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        self.layer_surface.destroy();
        self.surface.destroy();
    }
}

enum Action {
    Execute,
    Exit,
    Search,
    Complete,
    NavUp,
    NavDown,
}

struct DData {
    query: String,
    action: Option<Action>,
    modifiers: ModifiersState,
    clipboard: Clipboard,
}

pub fn main() {
    let maybe_config = config::Config::load();
    if let Err(e) = maybe_config {
        println!("{}", e);
        process::exit(1);
    }
    let config = maybe_config.unwrap();

    let (env, display, queue) =
        new_default_environment!(Env, fields = [layer_shell: SimpleGlobal::new(),])
            .expect("Initial roundtrip failed!");

    let (maybe_history, maybe_applications, font) =
        block_on(get_history_and_executables_and_font(&config));

    let history = maybe_history.unwrap_or_default();
    let mut applications = maybe_applications.unwrap();
    for app in history.keys() {
        if !applications.contains(app) {
            applications.push(app.to_string());
        }
    }
    applications.sort();

    let layer_shell = env.require_global::<zwlr_layer_shell_v1::ZwlrLayerShellV1>();
    let pools = env
        .create_double_pool(|_| {})
        .expect("Failed to create a memory pool!");
    let surface = env.create_surface().detach();
    let mut surface = Surface::new(None, surface, &layer_shell, pools);

    let mut event_loop = calloop::EventLoop::<DData>::new().unwrap();
    WaylandSource::new(queue)
        .quick_insert(event_loop.handle())
        .unwrap();

    let mut seats = Vec::<(
        String,
        Option<(wl_keyboard::WlKeyboard, calloop::Source<_>)>,
    )>::new();

    // first process already existing seats
    for seat in env.get_all_seats() {
        if let Some((has_ptr, name)) = with_seat_data(&seat, |seat_data| {
            (
                seat_data.has_pointer && !seat_data.defunct,
                seat_data.name.clone(),
            )
        }) {
            if has_ptr {
                let pointer = seat.get_pointer();
                pointer.quick_assign(move |_, event, ddata| process_pointer_event(event, ddata));
            } else {
                seats.push((name, None));
            }
        }
    }

    // first process already existing seats
    for seat in env.get_all_seats() {
        if let Some((has_kbd, name)) = with_seat_data(&seat, |seat_data| {
            (
                seat_data.has_keyboard && !seat_data.defunct,
                seat_data.name.clone(),
            )
        }) {
            if has_kbd {
                match map_keyboard_repeat(
                    event_loop.handle(),
                    &seat,
                    None,
                    RepeatKind::System,
                    move |event, _, ddata| process_keyboard_event(event, ddata),
                ) {
                    Ok((kbd, repeat_source)) => {
                        seats.push((name, Some((kbd, repeat_source))));
                    }
                    Err(e) => {
                        eprintln!("Failed to map keyboard on seat {} : {:?}.", name, e);
                        seats.push((name, None));
                    }
                }
            } else {
                seats.push((name, None));
            }
        }
    }

    let mut matched_exe = fuzzy_sort(&applications, "", &history);
    let mut need_redraw = false;
    let clipboard = unsafe { Clipboard::new(display.get_display_ptr() as *mut _) };
    let mut data: DData = DData {
        query: "".to_string(),
        action: None,
        modifiers: ModifiersState::default(),
        clipboard: clipboard,
    };
    let mut selection = 0;
    let mut select_query = false;

    loop {
        let DData { query, action, .. } = &mut data;
        match surface.next_render_event.take() {
            Some(RenderEvent::Closed) => break,
            Some(RenderEvent::Configure { width, height }) => {
                if surface.dimensions != (width, height) {
                    surface.dimensions = (width, height);
                    need_redraw = true;
                }
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
                    if select_query && matched_exe.len() > 0 {
                        select_query = false;
                    } else if matched_exe.len() > 0 && selection < matched_exe.len() - 1 {
                        selection += 1;
                    }
                }
                Action::Search => {
                    need_redraw = true;
                    matched_exe = fuzzy_sort(&applications, query, &history);
                    selection = 0;
                    if matched_exe.len() == 0 {
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
                            Ok(ForkResult::Parent { .. }) => {
                                let mut history = history.clone();
                                history.insert(
                                    query.to_string(),
                                    history.get(&query).unwrap_or(&0) + 1,
                                );
                                match history::commit_history(&history) {
                                    Ok(_) => {}
                                    Err(e) => {
                                        println!("{}", e.to_string())
                                    }
                                };
                            }
                            Ok(ForkResult::Child) => {
                                let err = exec::Command::new(args.remove(0)).args(&args).exec();
                                println!("Error: {}", err);
                            }
                            Err(_) => {
                                println!("failed to fork");
                            }
                        }
                        break;
                    }
                }
                Action::Exit => break,
            }
        }

        if need_redraw {
            need_redraw = false;

            // TODO: move this mess to it's own function
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
                    &query,
                    color,
                    &mut img,
                    config.padding + prompt_width,
                    config.padding,
                );
            }

            let spacer = (1.5 * font.scale.y) as u32;
            let max_entries = ((surface.dimensions.1 - 2 * config.padding - spacer) as f32
                / font.scale.y) as usize;
            let offset = if selection > (max_entries / 2) {
                (selection - max_entries / 2) as usize
            } else {
                0
            };

            for i in offset..(cmp::min(max_entries + offset, matched_exe.len())) {
                let color = if i == selection && !select_query {
                    &config.colors.text_selected
                } else {
                    &config.colors.text
                };
                font.render(
                    &matched_exe[i],
                    color,
                    &mut img,
                    config.padding,
                    (config.padding + spacer + (i - offset) as u32 * config.font_size as u32)
                        as u32,
                );
            }

            match surface.draw(&img) {
                Ok(_) => {}
                Err(e) => {
                    println!("{}", e);
                    need_redraw = false;
                }
            };
        }

        display.flush().unwrap();
        event_loop.dispatch(None, &mut data).unwrap();
    }
}

async fn get_history_and_executables_and_font(
    config: &config::Config,
) -> (
    Option<HashMap<String, usize>>,
    Option<Vec<String>>,
    font::Font<'_>,
) {
    join!(
        history::get_history_async(),
        get_executable_names(),
        font::Font::new_async(&config.font, config.font_size)
    )
}

async fn get_executable_names() -> Option<Vec<String>> {
    let var = match env::var_os("PATH") {
        Some(var) => var,
        None => return None,
    };

    let mut res: Vec<String> = Vec::new();

    let paths_iter = env::split_paths(&var);
    let dirs_iter = paths_iter.filter_map(|path| fs::read_dir(path).ok());

    for dir in dirs_iter {
        let executables_iter = dir
            .filter_map(|file| file.ok())
            .filter(|file| is_executable::is_executable(file.path()))
            .filter(|file| !file.path().is_dir());

        for exe in executables_iter {
            res.push(exe.file_name().to_str().unwrap().to_string());
        }
    }

    Some(res)
}

fn fuzzy_sort<'a>(
    executables: &'a Vec<String>,
    pattern: &str,
    pre_scored: &'a HashMap<String, usize>,
) -> Vec<&'a String> {
    let matcher = SkimMatcherV2::default();
    let mut executables = executables
        .into_iter()
        .map(|x| {
            (
                if let Some(score) = matcher.fuzzy_match(&x, &pattern) {
                    Some(score + *pre_scored.get(x).unwrap_or(&1) as i64)
                } else {
                    None
                },
                x,
            )
        })
        .collect::<Vec<(Option<i64>, &String)>>();
    executables.sort_by(|a, b| b.0.unwrap_or(0).cmp(&a.0.unwrap_or(0)));
    executables
        .into_iter()
        .filter(|x| x.0.is_some())
        .into_iter()
        .map(|x| x.1)
        .collect()
}

fn process_pointer_event(event: PEvent, mut data: DispatchData) {
    let DData {
        query,
        action,
        clipboard,
        ..
    } = data.get::<DData>().unwrap();
    match event {
        PEvent::Button { button, state, .. } => {
            if button == 274 && state == ButtonState::Pressed {
                if let Ok(txt) = clipboard.load_primary() {
                    query.push_str(&txt);
                    *action = Some(Action::Search);
                }
            }
        }
        _ => (),
    }
}

fn process_keyboard_event(event: KbEvent, mut data: DispatchData) {
    let DData {
        query,
        action,
        modifiers,
        clipboard,
        ..
    } = data.get::<DData>().unwrap();
    match event {
        KbEvent::Enter { .. } => {}
        KbEvent::Leave { .. } => {
            *action = Some(Action::Exit);
        }
        KbEvent::Key {
            keysym,
            state,
            utf8,
            ..
        } => {
            if modifiers.ctrl {
                match (state, keysym) {
                    (KeyState::Pressed, keysyms::XKB_KEY_v) => {
                        if let Ok(txt) = clipboard.load() {
                            query.push_str(&txt);
                            *action = Some(Action::Search);
                        }
                    }
                    _ => (),
                }
            } else {
                match (state, keysym) {
                    (KeyState::Pressed, keysyms::XKB_KEY_BackSpace) => {
                        query.pop();
                        *action = Some(Action::Search);
                    }
                    (KeyState::Pressed, keysyms::XKB_KEY_Tab) => {
                        *action = Some(Action::Complete);
                    }
                    (KeyState::Pressed, keysyms::XKB_KEY_Return) => {
                        *action = Some(Action::Execute);
                    }
                    (KeyState::Pressed, keysyms::XKB_KEY_Up) => {
                        *action = Some(Action::NavUp);
                    }
                    (KeyState::Pressed, keysyms::XKB_KEY_Down) => {
                        *action = Some(Action::NavDown);
                    }
                    (KeyState::Pressed, keysyms::XKB_KEY_Escape) => {
                        *action = Some(Action::Exit);
                    }
                    _ => {
                        if let Some(txt) = utf8 {
                            query.push_str(&txt);
                            *action = Some(Action::Search);
                        }
                    }
                }
            }
        }
        KbEvent::Modifiers { modifiers: m } => *modifiers = m,
        KbEvent::Repeat { .. } => {}
    }
}
