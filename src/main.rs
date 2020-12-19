use smithay_client_toolkit::{
    default_environment,
    environment::SimpleGlobal,
    new_default_environment,
    reexports::{
        calloop,
        client::protocol::{wl_output, wl_shm, wl_surface, wl_keyboard},
        client::{Attached, Main, DispatchData},
        protocols::wlr::unstable::layer_shell::v1::client::{
            zwlr_layer_shell_v1, zwlr_layer_surface_v1,
        },
    },
    shm::DoubleMemPool,
    WaylandSource,
    seat::{
        keyboard::{keysyms, map_keyboard_repeat, Event as KbEvent, RepeatKind, KeyState},
        with_seat_data,
    },
};

use byteorder::{NativeEndian, WriteBytesExt};

use std::cell::Cell;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::rc::Rc;

use image::{RgbaImage, Pixel, ImageBuffer};

use std::{env, fs, cmp};
use std::collections::HashMap;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

use nix::unistd::{fork, ForkResult};

mod history;
mod cli;
mod font;
mod color;

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
            zwlr_layer_shell_v1::Layer::Top,
            "launcher".to_owned(),
        );

        // Anchor to the top left corner of the output
        layer_surface
            .set_anchor(zwlr_layer_surface_v1::Anchor::all());
        
        // Enable Keyboard interactivity
        layer_surface
            .set_keyboard_interactivity(1);

        let next_render_event = Rc::new(Cell::new(None::<RenderEvent>));
        let next_render_event_handle = Rc::clone(&next_render_event);
        layer_surface.quick_assign(move |layer_surface, event, _| {
            match (event, next_render_event_handle.get()) {
                (zwlr_layer_surface_v1::Event::Closed, _) => {
                    next_render_event_handle.set(Some(RenderEvent::Closed));
                }
                (zwlr_layer_surface_v1::Event::Configure { serial, width, height }, next)
                    if next != Some(RenderEvent::Closed) =>
                {
                    layer_surface.ack_configure(serial);
                    next_render_event_handle.set(Some(RenderEvent::Configure { width, height }));
                }
                (_, _) => {}
            }
        });

        // Commit so that the server will send a configure event
        surface.commit();

        Self { surface, layer_surface, next_render_event, pools, dimensions: (0, 0) }
    }

    fn draw(&mut self, image: &RgbaImage) -> Result<(), std::io::Error>{
        // Note: unwrap() is only used here in the interest of simplicity of the example.
        // A "real" application should handle the case where both pools are still in use by the
        // compositor.
        let pool = self.pools.pool().unwrap();

        let stride = 4 * self.dimensions.0 as i32;
        let width = self.dimensions.0 as i32;
        let height = self.dimensions.1 as i32;

        // First make sure the pool is the right size
        pool.resize((stride * height) as usize)?;

        // Create a new buffer from the pool
        let buffer = pool.buffer(0, width, height, stride, wl_shm::Format::Argb8888);

        // Write the color to all bytes of the pool
        pool.seek(SeekFrom::Start(0))?;
        {
            let mut writer = BufWriter::new(&mut *pool);
            for p in image.pixels() {
                let c: (u8, u8, u8, u8) = p.channels4();
                writer.write_u32::<NativeEndian>(u32::from_le_bytes([c.2, c.1, c.0, c.3]) as u32)?;
            }
            writer.flush()?;
        }

        // Attach the buffer to the surface and mark the entire surface as damaged
        self.surface.attach(Some(&buffer), 0, 0);
        self.surface.damage_buffer(0, 0, width as i32, height as i32);

        // Finally, commit the surface
        self.surface.commit();
        Ok(())
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

type DData<'a> = (String, Option<Action>);
pub fn main() {
    //let matches = cli::build_cli().get_matches();

    let history = history::get_history().unwrap_or_default();
    let mut applications = get_executable_names().unwrap();
    for app in history.keys() {
        if !applications.contains(app) {
        applications.push(app.to_string());
        }
    }
    applications.sort();

    let matches = cli::build_cli().get_matches();

    let history = history::get_history().unwrap_or_default();
    let mut applications = get_executable_names().unwrap();
    for app in history.keys() {
        if !applications.contains(app) {
        applications.push(app.to_string());
        }
    }
    applications.sort();

    let color_background = color::Color::from(matches.value_of("background-color").unwrap().parse::<css_color::Rgba>().unwrap());

    let font_size = 32.0;
    let padding = 50;
    let (env, display, queue) =
        new_default_environment!(Env, fields = [layer_shell: SimpleGlobal::new(),])
            .expect("Initial roundtrip failed!");

    let layer_shell = env.require_global::<zwlr_layer_shell_v1::ZwlrLayerShellV1>();
    let pools = env.create_double_pool(|_| {}).expect("Failed to create a memory pool!");
    let surface = env.create_surface().detach();
    let mut surface = Surface::new(None, surface, &layer_shell, pools);

    let mut event_loop = calloop::EventLoop::<DData>::new().unwrap();
    WaylandSource::new(queue).quick_insert(event_loop.handle()).unwrap();

    let mut seats = Vec::<(String, Option<(wl_keyboard::WlKeyboard, calloop::Source<_>)>)>::new();
    // first process already existing seats
    for seat in env.get_all_seats() {
        if let Some((has_kbd, name)) = with_seat_data(&seat, |seat_data| {
        (seat_data.has_keyboard && !seat_data.defunct, seat_data.name.clone())
        }) {
        if has_kbd {
            let seat_name = name.clone();
            match map_keyboard_repeat(
            event_loop.handle(),
            &seat,
            None,
            RepeatKind::System,
            move |event, _, ddata| process_keyboard_event(event, &seat_name, ddata),
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

    let font_data = include_bytes!("../Roboto-Regular.ttf");
    let font = font::Font {
        font: rusttype::Font::try_from_bytes(font_data as &[u8]).expect("Error constructing Font"),
        scale: rusttype::Scale::uniform(32.),
    };

    let mut matched_exe = fuzzy_sort(&applications, "", &history);
    let mut need_redraw = false;
    let mut data: DData = ("".to_string(), None);
    let mut selection = 0;
    loop {
        let (query, next_action) = &mut data;
        match surface.next_render_event.take() {
            Some(RenderEvent::Closed) => break,
            Some(RenderEvent::Configure {width, height}) => {
                if surface.dimensions != (width, height) {
                    surface.dimensions = (width, height);
                    need_redraw = true;
                }
            },
            None => {}
        }
        if let Some(action) = next_action.take() {
            match action {
                Action::NavUp => {
                need_redraw = true;
                if selection > 0 {selection -=1; }
                },
                Action::NavDown => {
                need_redraw = true;
                if selection < matched_exe.len() - 1 {selection += 1;}
                },
                Action::Search => {
                need_redraw = true;
                matched_exe = fuzzy_sort(&applications, query, &history);
                selection = 0;
                },
                Action::Complete => {
                if let Some(app) = matched_exe.get(0) {
                    query.clear();
                    query.push_str(app);
                    matched_exe = fuzzy_sort(&applications, query, &history);
                    need_redraw = true;
                    selection = 0;
                }
                },
                Action::Execute => {
                if let Some(matched) = matched_exe.get(selection) {
                    match unsafe{ fork() } {
                    Ok(ForkResult::Parent {..}) => {
                        let mut history = history.clone();
                        history.insert(matched.to_string(), history.get(*matched).unwrap_or(&0) + 1);
                        match history::commit_history(&history) {
                        Ok(_) => {},
                        Err(e) => {println!("{}", e.to_string())}
                        };
                    },
                    Ok(ForkResult::Child) => {
                        let err = exec::Command::new(matched).exec();
                        println!("Error: {}", err); // TODO: show that in ui
                    }
                    Err(_) => {
                        println!("failed to fork");
                    }
                    }
                    break;
                } else if let Ok(mut args) = shellwords::split(query) {
                    if args.len() >= 1 {
                    match unsafe{ fork() } {
                        Ok(ForkResult::Parent {..}) => {
                        let mut history = history.clone();
                        history.insert(query.to_string(), history.get(query).unwrap_or(&0) + 1);
                        match history::commit_history(&history) {
                            Ok(_) => {},
                            Err(e) => {println!("{}", e.to_string())}
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
                },
                Action::Exit => break,
            }
        }

        if need_redraw {
            need_redraw = false;

            // TODO: move this mess to it's own function
            let mut img = ImageBuffer::from_pixel(surface.dimensions.0, surface.dimensions.1, color_background.to_rgba());
            if !query.is_empty() {
                let text_image: RgbaImage = font.render(&query, (152,195,121));
                image::imageops::overlay(&mut img, &text_image, padding, padding);
            }

            let spacer = (1.5 * font_size) as u32;
            let max_entries = ((surface.dimensions.1 - 2 * padding - spacer) as f32 / font_size) as usize;
            let offset = if selection > (max_entries / 2) {
                (selection - max_entries / 2) as usize
            } else {0};

            for i in offset..(cmp::min(max_entries + offset, matched_exe.len())) {
                let color = if i == selection {(97,175,239)} else {(255, 255, 255)};
                let text_image: RgbaImage = font.render(&matched_exe[i], color);
                image::imageops::overlay(&mut img, &text_image, padding, (padding + spacer + (i - offset) as u32 * text_image.height()) as u32);

            }

            surface.draw(&img);
        }

        display.flush().unwrap();
        event_loop.dispatch(None, &mut data).unwrap();
    }
}


fn get_executable_names() -> Option<Vec<String>> {
  let var = match env::var_os("PATH") {
    Some(var) => var,
    None => return None,
  };

  let mut res: Vec<String> = Vec::new();

  let paths_iter = env::split_paths(&var);
  let dirs_iter = paths_iter.filter_map(|path| fs::read_dir(path).ok());

  for dir in dirs_iter {
    let executables_iter = dir.filter_map(|file| file.ok())
        .filter(|file| is_executable::is_executable(file.path()))
        .filter(|file| !file.path().is_dir());
    
    for exe in executables_iter {
      res.push(exe.file_name().to_str().unwrap().to_string());
    }
  }

  Some(res)
} 

fn fuzzy_sort<'a>(executables: &'a Vec<String>, pattern: &str, pre_scored: &'a HashMap<String, usize>) -> Vec<&'a String> {
  let matcher = SkimMatcherV2::default();
  let mut executables = executables.into_iter()
    .map(|x| (if let Some(score) = matcher.fuzzy_match(&x , &pattern) {Some(score + *pre_scored.get(x).unwrap_or(&1) as i64)} else {None}, x))
    .collect::<Vec<(Option<i64>, &String)>>();
  executables.sort_by(|a, b| b.0.unwrap_or(0).cmp(&a.0.unwrap_or(0)));
  executables.into_iter().filter(|x| x.0.is_some()).into_iter().map(|x| x.1).collect()
}

fn process_keyboard_event(event: KbEvent, _seat_name: &str, mut data: DispatchData) {
  let (search, action) = data.get::<DData>().unwrap();
    match event {
        KbEvent::Enter { .. } => { }
        KbEvent::Leave { .. } => {
          *action = Some(Action::Exit);
        }
        KbEvent::Key { keysym, state, utf8, .. } => {
            match (state, keysym) {
              (KeyState::Pressed, keysyms::XKB_KEY_BackSpace) => {
                search.pop();
                *action = Some(Action::Search);
              },
              (KeyState::Pressed, keysyms::XKB_KEY_Tab) => {
                *action = Some(Action::Complete);
              },
              (KeyState::Pressed, keysyms::XKB_KEY_Return) => {
                *action = Some(Action::Execute);
              },
              (KeyState::Pressed, keysyms::XKB_KEY_Up) => {
                *action = Some(Action::NavUp);
              },
              (KeyState::Pressed, keysyms::XKB_KEY_Down) => {
                *action = Some(Action::NavDown);
              },
              (KeyState::Pressed, keysyms::XKB_KEY_Escape) => {
                *action = Some(Action::Exit);
              },
              _ => {
                if let Some(txt) = utf8 {
                  search.push_str(&txt);
                  *action = Some(Action::Search);
                }
              }
            }
        }
        KbEvent::Modifiers { .. } => {}
        KbEvent::Repeat { .. } => { }
    }
}