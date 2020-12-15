extern crate smithay_client_toolkit as sctk;
extern crate byteorder;
extern crate rusttype;
extern crate image;
extern crate is_executable;
extern crate shellwords;
extern crate clap;
extern crate css_color;
extern crate exec;

use sctk::window::{ConceptFrame, Event as WEvent, Decorations};
use sctk::reexports::client::protocol::{wl_keyboard, wl_shm, wl_surface};
use sctk::reexports::client::DispatchData;
use sctk::reexports::calloop;
use sctk::seat::keyboard::{map_keyboard_repeat, Event as KbEvent, RepeatKind, KeyState};
use sctk::seat::keyboard::keysyms;
use sctk::shm::MemPool;

use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::{env, fs, cmp};

use byteorder::{NativeEndian, WriteBytesExt};
use rusttype::{point, Font, Scale};
use image::{RgbaImage, ImageBuffer, Rgba, Pixel};
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use nix::unistd::{fork, ForkResult};
use clap::{Arg, App, crate_version, crate_authors};

sctk::default_environment!(Launcher, desktop);

enum Action {
  Execute,
  Exit,
  Search,
  Complete,
  NavUp,
  NavDown,
}

type DData<'a> = (Option<WEvent>, String, Option<Action>);

fn num_validator (num_str: String) -> Result<(), String> {
  match num_str.parse::<u32>() {
    Ok(_) => Ok(()),
    Err(e) => Err(e.to_string()),
  }
}

fn hex_validator (hex_str: String) -> Result<(), String> {
  match hex_str.parse::<css_color::Rgba>() {
    Ok(_) => Ok(()),
    Err(_) => Err("color parsing error".to_string()),
  }
}

fn main() {

  let matches = App::new("Kickoff")
    .version(crate_version!())
    .author(crate_authors!())
    .about("Minimal program launcher, focused on usability and speed")
    .arg(Arg::with_name("width")
      .short("w")
      .long("width")
      .value_name("PIXEL")
      .validator(num_validator)
      .help("Set window width"))
    .arg(Arg::with_name("heigth")
      .short("h")
      .long("heigth")
      .value_name("PIXEL")
      .validator(num_validator)
      .help("Set window heigth"))
    .arg(Arg::with_name("background")
      .long("background")
      .value_name("COLOR")
      .validator(hex_validator)
      .help("Background color"))
    .get_matches();
  
  let heigth: u32 = matches.value_of("heigth").unwrap_or("600").parse().unwrap();
  let width: u32 = matches.value_of("width").unwrap_or("800").parse().unwrap();
  let mut dimensions = (width, heigth);
  let color_background = matches.value_of("background").unwrap_or("#222222ff").parse::<css_color::Rgba>().unwrap();
  let color_background = Rgba([
    (color_background.red * 255.) as u8 ,
    (color_background.green * 255.) as u8 ,
    (color_background.blue * 255.) as u8 ,
    (color_background.alpha * 255.) as u8]);

  let font_size = 32.0;
  let padding = 50;

  let mut applications = get_executable_names().unwrap();
  applications.sort();
  
  let mut event_loop = calloop::EventLoop::<DData>::new().unwrap();

  let mut seats = Vec::<(String, Option<(wl_keyboard::WlKeyboard, calloop::Source<_>)>)>::new();

  // Window stuff
  let (env, display, queue) = sctk::new_default_environment!(Launcher, desktop)
    .expect("Unable to connect to the Wayland server");

  let surface = env.create_surface().detach();

  let mut window = env.create_window::<ConceptFrame, _>(
    surface,
    None,
    dimensions,
    move |evt, mut dispatch_data| {
      let (next_action, _, _) = dispatch_data.get::<DData>().unwrap();
      let replace = match (&evt, &*next_action) {
        (_, &None)
        | (_, &Some(WEvent::Refresh))
        | (&WEvent::Configure { .. }, &Some(WEvent::Configure  { .. }))
        | (&WEvent::Close, _) => true,
        _ => false,
      };
      if replace {
        *next_action = Some(evt);
      }
    },
  ).expect("Failed to create a window");

  window.set_title("Kickoff".to_string());
  window.set_resizable(false);
  window.set_decorate(Decorations::ClientSide);

  let mut pools = env.create_double_pool(|_| {}).expect("Failed to create the memory pools.");

  // first process already existing seats
  for seat in env.get_all_seats() {
    if let Some((has_kbd, name)) = sctk::seat::with_seat_data(&seat, |seat_data| {
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


  let mut need_redraw = false;

  if !env.get_shell().unwrap().needs_configure() {
    if let Some(pool) = pools.pool() {
      redraw(pool, window.surface(), dimensions, &ImageBuffer::from_pixel(dimensions.0, dimensions.1, color_background)).expect("Failed to draw");
    }
    window.refresh();
  }

  let mut matched_exe = fuzzy_sort(&applications, "");
  let mut data: DData = (None, "".to_string(), None);
  let mut selection = 0;
  sctk::WaylandSource::new(queue).quick_insert(event_loop.handle()).unwrap();

  loop {
    let (event, query, action) = &mut data;
    match event.take() {
      Some(WEvent::Close) => break,
      Some(WEvent::Refresh) => {
        window.refresh();
        window.surface().commit();
      },
      Some(WEvent::Configure {new_size, states: _}) => {
        if let Some((w, h)) = new_size {
          if dimensions != (w, h) {
            dimensions = (w, h);
          }
        }
        window.resize(dimensions.0, dimensions.1);
        window.refresh();

        need_redraw = true;
      },
      None => {},
    }

    if let Some(action) = action.take() {
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
          matched_exe = fuzzy_sort(&applications, query);
          selection = 0;
        },
        Action::Complete => {
          if let Some(app) = matched_exe.get(0) {
            query.clear();
            query.push_str(app);
            matched_exe = fuzzy_sort(&applications, query);
            need_redraw = true;
            selection = 0;
          }
        },
        Action::Execute => {
          if let Some(matched) = matched_exe.get(selection) {
            match unsafe{ fork() } {
              Ok(ForkResult::Parent {..}) => {},
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
                Ok(ForkResult::Parent {..}) => {},
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

      // TODO: define elsewhere
      let font_data = include_bytes!("../Roboto-Regular.ttf");
      let font = Font::try_from_bytes(font_data as &[u8]).expect("Error constructing Font");
      
      match pools.pool() {
        Some(pool) => {
          need_redraw = false;

          // TODO: move this mess to it's own function
          let mut img = ImageBuffer::from_pixel(dimensions.0, dimensions.1, color_background);
          if !query.is_empty() {
            let text_image: RgbaImage = render_text(&query, &font, Scale::uniform(font_size), (152,195,121));
            image::imageops::overlay(&mut img, &text_image, padding, padding);
          }

          let spacer = (1.5 * font_size) as u32;
          let max_entries = ((dimensions.1 - 2 * padding - spacer) as f32 / font_size) as usize;
          let offset = if selection > (max_entries / 2) {
            (selection - max_entries / 2) as usize
          } else {0};

          for i in offset..(cmp::min(max_entries + offset, matched_exe.len())) {
            let color = if i == selection {(97,175,239)} else {(255, 255, 255)};
            let text_image: RgbaImage = render_text(&matched_exe[i], &font, Scale::uniform(font_size), color);
            image::imageops::overlay(&mut img, &text_image, padding, (padding + spacer + (i - offset) as u32 * text_image.height()) as u32);

          }

          redraw(
            pool,
            window.surface(),
            dimensions,
            &img).expect("Failed to draw")
        }
        None => {}
      }
    }

    display.flush().unwrap();
    event_loop.dispatch(None, &mut data).unwrap();
  }
}

fn render_text(text: &str, font: &rusttype::Font, scale: rusttype::Scale, colour: (u8, u8, u8)) -> RgbaImage {
  let v_metrics = font.v_metrics(scale);

  let glyphs: Vec<_> = font.layout(text, scale, point(0.0, v_metrics.ascent)).collect();
  let glyphs_height = (v_metrics.ascent - v_metrics.descent).ceil() as u32;
  let glyphs_width = glyphs
        .iter()
        .rev()
        .map(|g| g.position().x as f32 + g.unpositioned().h_metrics().advance_width)
        .next()
        .unwrap_or(0.0)
        .ceil() as u32;

  let mut image = RgbaImage::new(glyphs_width, glyphs_height);
  for glyph in glyphs {
    if let Some(bounding_box) = glyph.pixel_bounding_box() {
      glyph.draw(|x, y, v| {
        let x = x + bounding_box.min.x as u32;
        let y = y + bounding_box.min.y as u32;
        if x < glyphs_width && y < glyphs_height {
          image.put_pixel(
            x,
            y,
            Rgba([colour.0, colour.1, colour.2, (v * 255.0) as u8]),
          )
        }
      });
    }
  }
  return image;
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

fn redraw(
  pool: &mut MemPool,
  surface: &wl_surface::WlSurface,
  (buf_x, buf_y): (u32, u32),
  image: &RgbaImage,
) -> Result<(), ::std::io::Error> {
  pool.resize((4 * buf_x * buf_y) as usize).expect("Failed to resize the memory pool.");
  pool.seek(SeekFrom::Start(0))?;
  {
    let mut writer = BufWriter::new(&mut *pool);
    for p in image.pixels() {
      let c: (u8, u8, u8, u8) = p.channels4();
      writer.write_u32::<NativeEndian>(u32::from_le_bytes([c.2, c.1, c.0, c.3]) as u32)?;
    }
    writer.flush()?;
  }

  let new_buffer = pool.buffer(0, buf_x as i32, buf_y as i32, 4 * buf_x as i32, wl_shm::Format::Argb8888,);

  surface.attach(Some(&new_buffer), 0, 0);
  if surface.as_ref().version() >= 4 {
    surface.damage_buffer(0,0, buf_x as i32, buf_y as i32);
  } else {
    surface.damage(0,0, buf_x as i32, buf_y as i32);
  }
  surface.commit();
  Ok(())
}

fn fuzzy_sort<'a>(executables: &'a Vec<String>, pattern: &str) -> Vec<&'a String> {
  let matcher = SkimMatcherV2::default();
  let mut executables = executables.into_iter()
    .map(|x| (matcher.fuzzy_match(&x.to_lowercase() , &pattern.to_lowercase()), x))
    .collect::<Vec<(Option<i64>, &String)>>();
  executables.sort_by(|a, b| b.0.unwrap_or(0).cmp(&a.0.unwrap_or(0)));
  executables.into_iter().filter(|x| x.0.is_some()).into_iter().map(|x| x.1).collect()
}

fn process_keyboard_event(event: KbEvent, _seat_name: &str, mut data: DispatchData) {
  let (_, search, action) = data.get::<DData>().unwrap();
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