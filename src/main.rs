extern crate smithay_client_toolkit as sctk;
extern crate byteorder;
extern crate rusttype;
extern crate image;
extern crate is_executable;

use sctk::window::{ConceptFrame, Event as WEvent};
use std::io::{BufWriter, Seek, SeekFrom, Write};
use sctk::shm::MemPool;
use byteorder::{NativeEndian, WriteBytesExt};
use sctk::reexports::client::protocol::{wl_keyboard, wl_shm, wl_surface};
use sctk::reexports::client::DispatchData;
use rusttype::{point, Font, Scale};
use image::{RgbaImage, ImageBuffer, Rgba, Pixel};
use sctk::reexports::calloop;
use sctk::seat::keyboard::{map_keyboard_repeat, Event as KbEvent, RepeatKind, KeyState};
use smithay_client_toolkit::seat::keyboard::keysyms;
use std::env;
use std::fs::{self, DirEntry};
use std::cmp;
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;

sctk::default_environment!(Launcher, desktop);

enum Action {
  Execute,
  Exit,
  Search,
}

type DData = (Option<WEvent>, String, Option<Action>);

fn main() {

  let mut executables = get_executables().unwrap();
  executables.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
  
  let mut event_loop = calloop::EventLoop::<DData>::new().unwrap();

  let mut seats = Vec::<(String, Option<(wl_keyboard::WlKeyboard, calloop::Source<_>)>)>::new();

  // Window stuff
  let mut dimensions = (1920, 1080);
  let (env, display, queue) = sctk::new_default_environment!(Launcher, desktop)
    .expect("Unable to connect to the Wayland server");

  let surface = env.create_surface().detach();
  let base_img: RgbaImage = ImageBuffer::from_pixel(dimensions.0, dimensions.1, Rgba([0,0,0,200]));


  let mut window = env.create_window::<ConceptFrame, _>(
    surface,
    None,
    (1920,1080),
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

  window.set_title("WiniLauncher".to_string());
  window.set_resizable(false);

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
      redraw(pool, window.surface(), dimensions, &base_img).expect("Failed to draw");
    }
    window.refresh();
  }

  let mut data: DData = (None, "".to_string(), None);
  sctk::WaylandSource::new(queue).quick_insert(event_loop.handle()).unwrap();

  loop {
    match data.0.take() {
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

    if let Some(action) = data.2.take() {
      match action {
        Action::Search => need_redraw = true,
        Action::Exit => break,
        Action::Execute => break, // TODO: Implement ;)
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
          let mut img = base_img.clone();
          let search = &data.1;
          if !search.is_empty() {
            let text_image: RgbaImage = render_text(&search, &font, Scale::uniform(64.), (152,195,121));
            image::imageops::overlay(&mut img, &text_image, 10, 10);
          }

          let executables = fuzzy_sort(&executables, &search);
          for i in 0..(cmp::min(10, executables.len())) {
            let text_image: RgbaImage = render_text(&executables[i].file_name().to_str().unwrap(), &font, Scale::uniform(64.), (97,175,239));
            image::imageops::overlay(&mut img, &text_image, 10, (100 + i * text_image.height() as usize) as u32);

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

fn get_executables() -> Option<Vec<DirEntry>> {
  let var = match env::var_os("PATH") {
    Some(var) => var,
    None => return None,
  };

  let mut res: Vec<DirEntry> = Vec::new();

  let paths_iter = env::split_paths(&var);
  let dirs_iter = paths_iter.filter_map(|path| fs::read_dir(path).ok());

  for dir in dirs_iter {
    let executables_iter = dir.filter_map(|file| file.ok())
        .filter(|file| is_executable::is_executable(file.path()))
        .filter(|file| !file.path().is_dir());
    
    for exe in executables_iter {
      res.push(exe);
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

fn fuzzy_sort<'a>(executables: &'a Vec<DirEntry>, pattern: &str) -> Vec<&'a DirEntry> {
  let matcher = SkimMatcherV2::default();
  let mut executables = executables.into_iter().map(|x| (matcher.fuzzy_match(&x.file_name().into_string().ok().unwrap().to_lowercase() , pattern), x)).collect::<Vec<(Option<i64>, &DirEntry)>>();
  executables.sort_by(|a, b| b.0.unwrap_or(0).cmp(&a.0.unwrap_or(0)));
  executables.into_iter().filter(|x| x.0.is_some()).into_iter().map(|x| x.1).collect()
}

fn process_keyboard_event(event: KbEvent, seat_name: &str, mut data: DispatchData) {

  let (_, search, action) = data.get::<DData>().unwrap();
    match event {
        KbEvent::Enter { keysyms, .. } => {
            println!("Gained focus on seat '{}' while {} keys pressed.", seat_name, keysyms.len(),);
        }
        KbEvent::Leave { .. } => {
            println!("Lost focus on seat '{}'.", seat_name);
        }
        KbEvent::Key { keysym, state, utf8, .. } => {
            println!("Key {:?}: {:x} on seat '{}'.", state, keysym, seat_name);
            match (state, keysym) {
              (KeyState::Pressed, keysyms::XKB_KEY_BackSpace) => {
                search.pop();
                println!(" -> Backspace received");
                println!(" -> Text is now \"{}\".", search.to_string());
                *action = Some(Action::Search);
              },
              (KeyState::Pressed, keysyms::XKB_KEY_Return) => {
                *action = Some(Action::Execute);
              },
              (KeyState::Pressed, keysyms::XKB_KEY_Escape) => {
                *action = Some(Action::Exit);
              },
              _ => {
                if let Some(txt) = utf8 {
                  search.push_str(&txt);
                  println!(" -> Received text \"{}\".", txt);
                  println!(" -> Text is now \"{}\".", search.to_string());
                  *action = Some(Action::Search);
                }
              }
            }
        }
        KbEvent::Modifiers { .. } => {}
        KbEvent::Repeat { keysym, utf8, .. } => {
            println!("Key repetition {:x} on seat '{}'.", keysym, seat_name);
            if let Some(txt) = utf8 {
                println!(" -> Received text \"{}\".", txt);
            }
        }
    }
}