use crate::{keybinds::Keybindings, App};
use image::Pixel;
use log::{debug, error};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_keyboard, delegate_layer, delegate_output, delegate_pointer,
    delegate_registry, delegate_seat, delegate_shm,
    output::{OutputHandler, OutputState},
    reexports::{
        calloop::{EventLoop, LoopHandle},
        calloop_wayland_source::WaylandSource,
    },
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        keyboard::{KeyEvent, KeyboardHandler, Modifiers},
        pointer::{PointerEvent, PointerEventKind, PointerHandler},
        Capability, SeatHandler, SeatState,
    },
    shell::{
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
        WaylandSurface,
    },
    shm::{slot::SlotPool, Shm, ShmHandler},
};
use std::{
    io::{BufWriter, Read, Write},
    time::Duration,
};
use wayland_client::{
    globals::registry_queue_init,
    protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
    Connection, QueueHandle,
};
use wl_clipboard_rs::paste::{get_contents, ClipboardType, Error, MimeType, Seat};

#[derive(Clone)]
pub enum Action {
    Execute,
    Exit,
    Complete,
    NavUp,
    NavDown,
    Delete,
    DeleteWord,
    Paste,
    Insert(String),
}

pub fn run(app: App) {
    let conn = Connection::connect_to_env().unwrap();

    let (globals, event_queue) = registry_queue_init(&conn).unwrap();
    let qh = event_queue.handle();
    let mut event_loop: EventLoop<GuiLayer> =
        EventLoop::try_new().expect("Failed to initialize event loop");
    let loop_handle = event_loop.handle();
    WaylandSource::new(conn.clone(), event_queue)
        .insert(loop_handle)
        .unwrap();

    let compositor = CompositorState::bind(&globals, &qh).expect("wl_compositor is not available");
    let layer_shell = LayerShell::bind(&globals, &qh).expect("layer shell is not available");
    let shm = Shm::bind(&globals, &qh).expect("wl_shm is not available");

    let surface = compositor.create_surface(&qh);

    let layer = layer_shell.create_layer_surface(&qh, surface, Layer::Top, Some("kickoff"), None);

    layer.set_anchor(Anchor::all());
    layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);

    layer.commit();

    let pool = SlotPool::new(256 * 256 * 4, &shm).expect("Failed to create pool");

    let mut gui_layer = GuiLayer {
        registry_state: RegistryState::new(&globals),
        seat_state: SeatState::new(&globals, &qh),
        output_state: OutputState::new(&globals, &qh),
        shm,

        exit: false,
        first_configure: true,
        pool,
        width: 256,
        height: 256,
        layer,
        keyboard: None,
        pointer: None,
        scale_factor: 1,
        modifiers: Modifiers::default(),
        keybindings: Keybindings::from(app.config.keybindings.clone()),
        app,
        next_action: None,
        loop_handle: event_loop.handle(),
    };

    loop {
        event_loop
            .dispatch(Duration::from_millis(50), &mut gui_layer)
            .unwrap();
        match &gui_layer.next_action.take() {
            Some(Action::Exit) => gui_layer.exit = true,
            Some(Action::Complete) => gui_layer.app.complete(),
            Some(Action::Delete) => gui_layer.app.delete(),
            Some(Action::DeleteWord) => gui_layer.app.delete_word(),
            Some(Action::NavUp) => gui_layer.app.nav_up(1),
            Some(Action::NavDown) => gui_layer.app.nav_down(1),
            Some(Action::Insert(s)) => gui_layer.app.insert(s),
            Some(Action::Execute) => {
                gui_layer.app.execute();
                gui_layer.exit = true;
            }
            Some(Action::Paste) => {
                let result =
                    get_contents(ClipboardType::Regular, Seat::Unspecified, MimeType::Text);
                match result {
                    Ok((mut pipe, _)) => {
                        let mut contents = vec![];
                        pipe.read_to_end(&mut contents).unwrap();
                        let input = String::from_utf8(contents).unwrap();
                        gui_layer.app.insert(&input);
                    }
                    Err(Error::NoSeats | Error::ClipboardEmpty | Error::NoMimeType) => {}
                    Err(e) => error!("{e}"),
                }
            }
            _ => {}
        }

        if gui_layer.exit {
            debug!("exiting kickoff");
            break;
        }
    }
}

struct GuiLayer {
    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    shm: Shm,

    exit: bool,
    first_configure: bool,
    pool: SlotPool,
    width: u32,
    height: u32,
    layer: LayerSurface,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    pointer: Option<wl_pointer::WlPointer>,
    scale_factor: i32,
    modifiers: Modifiers,
    app: App,
    next_action: Option<Action>,
    keybindings: Keybindings,
    loop_handle: LoopHandle<'static, GuiLayer>,
}

impl CompositorHandler for GuiLayer {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        new_factor: i32,
    ) {
        self.scale_factor = new_factor;
        self.layer.set_buffer_scale(new_factor as u32).unwrap();
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
        self.draw(qh);
    }

    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wl_output::Transform,
    ) {
    }
}

impl OutputHandler for GuiLayer {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }
}

impl LayerShellHandler for GuiLayer {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {
        self.exit = true;
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        if configure.new_size.0 == 0 || configure.new_size.1 == 0 {
            self.width = 256;
            self.height = 256;
        } else {
            self.width = configure.new_size.0;
            self.height = configure.new_size.1;
        }

        // Initiate the first draw.
        if self.first_configure {
            self.first_configure = false;
            self.draw(qh);
        }
    }
}

impl SeatHandler for GuiLayer {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

    fn new_capability(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard && self.keyboard.is_none() {
            debug!("Set keyboard capability");
            let keyboard = self
                .seat_state
                .get_keyboard_with_repeat(
                    qh,
                    &seat,
                    None,
                    self.loop_handle.clone(),
                    Box::new(|state, _wl_kbd, event| {
                        if let Some(action) = state.keybindings.get(state.modifiers, event.keysym) {
                            state.next_action = Some(action.clone());
                        } else if let Some(input) = event.utf8 {
                            state.next_action = Some(Action::Insert(input));
                        }
                    }),
                )
                .expect("Failed to create keyboard");
            self.keyboard = Some(keyboard);
        }

        if capability == Capability::Pointer && self.pointer.is_none() {
            debug!("Set pointer capability");
            let pointer = self
                .seat_state
                .get_pointer(qh, &seat)
                .expect("Failed to create pointer");
            self.pointer = Some(pointer);
        }
    }

    fn remove_capability(
        &mut self,
        _conn: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard && self.keyboard.is_some() {
            debug!("Unset keyboard capability");
            self.keyboard.take().unwrap().release();
        }

        if capability == Capability::Pointer && self.pointer.is_some() {
            debug!("Unset pointer capability");
            self.pointer.take().unwrap().release();
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl KeyboardHandler for GuiLayer {
    fn enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: &wl_surface::WlSurface,
        _: u32,
        _: &[u32],
        _keysyms: &[smithay_client_toolkit::seat::keyboard::Keysym],
    ) {
    }

    fn leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: &wl_surface::WlSurface,
        _: u32,
    ) {
        self.next_action = Some(Action::Exit);
    }

    fn press_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        debug!("Key press: {event:?}");
        if let Some(action) = self.keybindings.get(self.modifiers, event.keysym) {
            self.next_action = Some(action.clone());
        } else if let Some(input) = event.utf8 {
            self.next_action = Some(Action::Insert(input));
        }
    }

    fn release_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        debug!("Key release: {event:?}");
    }

    fn update_modifiers(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _serial: u32,
        modifiers: Modifiers,
    ) {
        debug!("Update modifiers: {modifiers:?}");
        self.modifiers = modifiers;
    }
}

impl PointerHandler for GuiLayer {
    fn pointer_frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _pointer: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        use PointerEventKind::Press;
        for event in events {
            // Ignore events for other surfaces
            if &event.surface != self.layer.wl_surface() {
                continue;
            }

            if let Press { button: 274, .. } = event.kind {
                let result =
                    get_contents(ClipboardType::Primary, Seat::Unspecified, MimeType::Text);
                match result {
                    Ok((mut pipe, _)) => {
                        let mut contents = vec![];
                        pipe.read_to_end(&mut contents).unwrap();
                        let input = String::from_utf8(contents).unwrap();
                        self.next_action = Some(Action::Insert(input));
                    }
                    Err(Error::NoSeats | Error::ClipboardEmpty | Error::NoMimeType) => {}
                    Err(e) => error!("{e}"),
                }
            }
        }
    }
}

impl ShmHandler for GuiLayer {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl GuiLayer {
    pub fn draw(&mut self, qh: &QueueHandle<Self>) {
        let width = self.width * self.scale_factor as u32;
        let height = self.height * self.scale_factor as u32;
        let stride = width as i32 * 4;

        let (buffer, canvas) = self
            .pool
            .create_buffer(
                width as i32,
                height as i32,
                stride,
                wl_shm::Format::Argb8888,
            )
            .expect("create buffer");

        let mut image = self.app.draw(width, height, self.scale_factor);
        image.pixels_mut().for_each(|pixel| {
            let channels = pixel.channels_mut();
            channels.swap(0, 2);
        });

        // Draw to the window:
        let mut writer = BufWriter::new(&mut *canvas);
        writer.write_all(image.as_raw()).unwrap();
        writer.flush().unwrap();

        // Damage the entire window
        self.layer
            .wl_surface()
            .damage_buffer(0, 0, width as i32, height as i32);

        // Request our next frame
        self.layer
            .wl_surface()
            .frame(qh, self.layer.wl_surface().clone());

        // Attach and commit to present.
        buffer
            .attach_to(self.layer.wl_surface())
            .expect("buffer attach");
        self.layer.commit();
    }
}

delegate_compositor!(GuiLayer);
delegate_output!(GuiLayer);
delegate_shm!(GuiLayer);

delegate_seat!(GuiLayer);
delegate_keyboard!(GuiLayer);
delegate_pointer!(GuiLayer);

delegate_layer!(GuiLayer);

delegate_registry!(GuiLayer);

impl ProvidesRegistryState for GuiLayer {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState];
}
