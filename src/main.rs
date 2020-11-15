extern crate smithay_client_toolkit as sctk;
extern crate byteorder;
extern crate rusttype;
extern crate image;

use sctk::window::{ConceptFrame, Event as WEvent};
use std::io::{BufWriter, Seek, SeekFrom, Write};
use sctk::shm::MemPool;
use byteorder::{NativeEndian, WriteBytesExt};
use sctk::reexports::client::protocol::{wl_shm, wl_surface};
use rusttype::{point, Font, Scale};
use image::{RgbaImage, ImageBuffer, Rgba, Pixel};

sctk::default_environment!(Launcher, desktop);

fn main() {

  // Font stuff
  let font_data = include_bytes!("../Roboto-Regular.ttf");
  let font = Font::try_from_bytes(font_data as &[u8]).expect("Error constructing Font");
  

  // Window stuff
  let mut dimensions = (1920, 1080);
  let (env, _display, mut queue) = sctk::new_default_environment!(Launcher, desktop)
    .expect("Unable to connect to the Wayland server");

  let surface = env.create_surface().detach();
  let mut img: RgbaImage = ImageBuffer::from_pixel(dimensions.0, dimensions.1, Rgba([0,0,0,200]));

  let text_image: RgbaImage = render_text("Hello Wayland?", font, Scale::uniform(64.), (255,0,255));
  image::imageops::overlay(&mut img, &text_image, dimensions.0 / 2, dimensions.1 / 2);

  let mut next_action = None::<WEvent>;
  let mut window = env.create_window::<ConceptFrame, _>(
    surface,
    None,
    (1920,1080),
    move |evt, mut dispatch_data| {
      let next_action = dispatch_data.get::<Option<WEvent>>().unwrap();
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

  let mut need_redraw = false;

  if !env.get_shell().unwrap().needs_configure() {
    if let Some(pool) = pools.pool() {
      redraw(pool, window.surface(), dimensions, &img).expect("Failed to draw");
    }
    window.refresh();
  }

  loop {
    match next_action.take() {
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
      None => {}
    }

    if need_redraw { // in the future determine if redraw needed
      match pools.pool() {
        Some(pool) => {
          need_redraw = false;
          redraw(
            pool,
            window.surface(),
            dimensions,
            &img).expect("Failed to draw")
        }
        None => {}
      }
    }

    queue.dispatch(&mut next_action, |_, _, _| {}).unwrap();
  }

}

fn render_text(text: &str, font: rusttype::Font, scale: rusttype::Scale, colour: (u8, u8, u8)) -> RgbaImage {
  // Font stuff
  let v_metrics = font.v_metrics(scale);

  let glyphs: Vec<_> = font.layout(text, scale, point(0.0, v_metrics.ascent)).collect();
  let glyphs_height = (v_metrics.ascent - v_metrics.descent).ceil() as u32;
  let glyphs_width = {
    let min_x = glyphs
      .first()
      .map(|g| g.pixel_bounding_box().unwrap().min.x)
      .unwrap();
    let max_x = glyphs
      .last()
      .map(|g| g.pixel_bounding_box().unwrap().max.x)
      .unwrap();
    (max_x - min_x + 4) as u32 //TODO: +4 as safety margin, need to figure out where they're actually comming from
  };

  let mut image = RgbaImage::new(glyphs_width, glyphs_height);
  for glyph in glyphs {
    if let Some(bounding_box) = glyph.pixel_bounding_box() {
      glyph.draw(|x, y, v| {
        println!("{}", x + bounding_box.min.x as u32);
        image.put_pixel(
          x + bounding_box.min.x as u32,
          y + bounding_box.min.y as u32,
          Rgba([colour.0, colour.1, colour.2, (v * 255.0) as u8]),
        )
      });
    }
  }
  return image;
}

fn redraw(
  pool: &mut MemPool,
  surface: &wl_surface::WlSurface,
  (buf_x, buf_y): (u32, u32),
  image: &RgbaImage,
) -> Result<(), ::std::io::Error> {
  println!("x: {}, y: {}", buf_x, buf_y);
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
