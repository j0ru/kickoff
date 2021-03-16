use smithay_client_toolkit::{
    reexports::{
        calloop,
        client::protocol::{
            wl_output,
            wl_pointer::{ButtonState, Event as PEvent},
            wl_seat, wl_shm, wl_surface,
        },
        client::{Attached, DispatchData, Display, Main},
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
};

use smithay_clipboard::Clipboard;

use std::cell::Cell;
use std::io::{BufWriter, ErrorKind, Seek, SeekFrom, Write};
use std::rc::Rc;

use image::RgbaImage;

#[derive(PartialEq, Copy, Clone)]
pub enum RenderEvent {
    Configure { width: u32, height: u32 },
    Closed,
}

pub struct Surface {
    surface: wl_surface::WlSurface,
    layer_surface: Main<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1>,
    pub next_render_event: Rc<Cell<Option<RenderEvent>>>,
    pools: DoubleMemPool,
    pub dimensions: (u32, u32),
}

impl Surface {
    pub fn set_dimensions(&mut self, width: u32, height: u32) -> bool {
        if self.dimensions != (width, height) {
            self.dimensions = (width, height);
            true
        } else {
            false
        }
    }
    pub fn new(
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

    pub fn draw(&mut self, image: &RgbaImage) -> Result<(), std::io::Error> {
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

pub enum Action {
    Execute,
    Exit,
    Search,
    Complete,
    NavUp,
    NavDown,
}

pub struct DData {
    pub query: String,
    pub action: Option<Action>,
    pub modifiers: ModifiersState,
    pub clipboard: Clipboard,
}

impl DData {
    pub fn new(display: &Display) -> DData {
        let clipboard = unsafe { Clipboard::new(display.get_display_ptr() as *mut _) };
        DData {
            query: "".to_string(),
            action: None,
            modifiers: ModifiersState::default(),
            clipboard,
        }
    }
}

pub fn register_inputs(
    seats: &[Attached<wl_seat::WlSeat>],
    event_loop: &calloop::EventLoop<DData>,
) {
    for seat in seats {
        if let Some((has_ptr, _name)) = with_seat_data(&seat, |seat_data| {
            (
                seat_data.has_pointer && !seat_data.defunct,
                seat_data.name.clone(),
            )
        }) {
            if has_ptr {
                let pointer = seat.get_pointer();
                pointer.quick_assign(move |_, event, ddata| process_pointer_event(event, ddata));
            }
        }
    }

    for seat in seats {
        if let Some((has_kbd, name)) = with_seat_data(&seat, |seat_data| {
            (
                seat_data.has_keyboard && !seat_data.defunct,
                seat_data.name.clone(),
            )
        }) {
            if has_kbd {
                if let Err(err) = map_keyboard_repeat(
                    event_loop.handle(),
                    &seat,
                    None,
                    RepeatKind::System,
                    move |event, _, ddata| process_keyboard_event(event, ddata),
                ) {
                    eprintln!("Failed to map keyboard on seat {} : {:?}.", name, err)
                }
            }
        }
    }
}

fn process_pointer_event(event: PEvent, mut data: DispatchData) {
    let DData {
        query,
        action,
        clipboard,
        ..
    } = data.get::<DData>().unwrap();
    if let PEvent::Button { button, state, .. } = event {
        if button == 274 && state == ButtonState::Pressed {
            if let Ok(txt) = clipboard.load_primary() {
                query.push_str(&txt);
                *action = Some(Action::Search);
            }
        }
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
                if let (KeyState::Pressed, keysyms::XKB_KEY_v, Ok(txt)) =
                    (state, keysym, clipboard.load())
                {
                    query.push_str(&txt);
                    *action = Some(Action::Search);
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
        KbEvent::Repeat { keysym, utf8, .. } => match keysym {
            keysyms::XKB_KEY_BackSpace => {
                query.pop();
                *action = Some(Action::Search);
            }
            keysyms::XKB_KEY_Tab => {
                *action = Some(Action::Complete);
            }
            keysyms::XKB_KEY_Up => {
                *action = Some(Action::NavUp);
            }
            keysyms::XKB_KEY_Down => {
                *action = Some(Action::NavDown);
            }
            _ => {
                if let Some(txt) = utf8 {
                    query.push_str(&txt);
                    *action = Some(Action::Search);
                }
            }
        },
    }
}
