//! KeyOverlay - Rust rewrite of the SFML/.NET original.
//!
//! `config.txt` next to the executable (or the file named by the first CLI
//! argument) drives everything, exactly like the original. A second window
//! edits that configuration; it opens with `Ctrl+Alt+K` or by clicking the
//! overlay. Start-up problems are reported through the same files the original
//! used: `errorMessage.txt` for anything, `keyErrorMessage.txt` for invalid key
//! names.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(not(windows))]
compile_error!(
    "This rewrite targets Windows only: global key polling uses GetAsyncKeyState (src/input.rs). \
     Add a cross-platform backend to build it elsewhere."
);

mod config;
mod hotkey;
#[cfg(windows)]
mod input;
mod keys;
mod lang;
mod layout;
mod presenter;
mod render;
mod settings;
mod state;
mod text;
mod ui;

use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tiny_skia::Pixmap;
use windows_sys::Win32::Foundation::RECT;
use windows_sys::Win32::UI::WindowsAndMessaging::{SPI_GETWORKAREA, SystemParametersInfoW};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId, WindowLevel};

use config::Config;
use hotkey::UserEvent;
use lang::Language;
use layout::Square;
use presenter::Presenter;
use render::{Renderer, Scene};
use settings::Settings;
use state::KeySlot;
use text::TextPainter;

/// Bundled face, so the executable is self-contained.
const FONT: &[u8] = include_bytes!("../assets/consolab.ttf");
const SETTINGS_SIZE: (u32, u32) = (980, 780);
/// Base size bump of the settings window: at 100% DPI the original numbers came
/// out smaller than a normal Windows dialog, so everything is drawn 1.25x and
/// then multiplied by the display's own scale factor. `uiScale` in config.txt
/// zooms further, relative to the display, so it survives a monitor change.
const UI_BASE_SCALE: f32 = 1.1;

fn ui_scale(config: &Config, monitor_scale: f32) -> f32 {
    (UI_BASE_SCALE * monitor_scale * config.ui_scale).clamp(0.5, 4.0)
}

/// Height of the primary monitor's work area, so the settings window can never
/// grow taller than the screen.
fn work_area_height() -> Option<u32> {
    let mut rect: RECT = unsafe { std::mem::zeroed() };
    let ok = unsafe { SystemParametersInfoW(SPI_GETWORKAREA, 0, (&raw mut rect).cast(), 0) };
    (ok != 0).then(|| (rect.bottom - rect.top).max(0) as u32)
}

/// Font bytes shared by both windows. The embedded Consolas has no CJK glyphs,
/// so Chinese labels and Chinese `displayKey` text fall back to a system face
/// (see `lang::load_cjk_font`).
#[derive(Clone, Default)]
struct Fonts {
    cjk: Option<Arc<Vec<u8>>>,
}

impl Fonts {
    fn load() -> Self {
        Self {
            cjk: lang::load_cjk_font().map(Arc::new),
        }
    }

    fn painter(&self) -> Result<TextPainter, String> {
        TextPainter::with_fallback(FONT, self.cjk.as_ref().map(|bytes| bytes.as_slice()))
    }
}

enum StartupError {
    Config(String),
    InvalidKey(String),
}

/// Everything belonging to the overlay window.
struct Overlay {
    executable_dir: PathBuf,
    config: Config,
    squares: Vec<Square>,
    labels: Vec<String>,
    inputs: Vec<keys::Input>,
    slots: Vec<KeySlot>,
    pressed: Vec<bool>,
    renderer: Option<Renderer>,
    /// Created once: re-parsing the fallback face on every edit would be slow.
    text: TextPainter,
    input: input::InputState,
    window: Option<Arc<Window>>,
    presenter: Option<Presenter>,
    pixmap: Option<Pixmap>,
    last_frame: Instant,
    next_frame: Instant,
    /// `Duration::ZERO` means "no frame limit" (`maxFPS=0`).
    period: Duration,
}

impl Overlay {
    fn new(executable_dir: &Path, config_name: &str, fonts: Fonts) -> Result<Self, StartupError> {
        let config = config::load(executable_dir, config_name).map_err(StartupError::Config)?;
        let mut overlay = Self {
            executable_dir: executable_dir.to_path_buf(),
            config,
            squares: Vec::new(),
            labels: Vec::new(),
            inputs: Vec::new(),
            slots: Vec::new(),
            pressed: Vec::new(),
            renderer: None,
            text: fonts.painter().map_err(StartupError::Config)?,
            input: input::InputState::new(),
            window: None,
            presenter: None,
            pixmap: None,
            last_frame: Instant::now(),
            next_frame: Instant::now(),
            period: Duration::ZERO,
        };
        overlay.rebuild()?;
        Ok(overlay)
    }

    /// Rebuilds everything derived from the configuration. Called on start-up
    /// and after every settings edit.
    fn rebuild(&mut self) -> Result<(), StartupError> {
        let ratio_y = self.config.window_height as f32 / 960.0;

        let mut inputs = Vec::with_capacity(self.config.keys.len());
        let mut labels = Vec::with_capacity(self.config.keys.len());
        for (index, name) in self.config.keys.iter().enumerate() {
            let parsed = keys::parse(name).map_err(StartupError::InvalidKey)?;
            let display = self
                .config
                .display_keys
                .get(index)
                .map(String::as_str)
                .unwrap_or_default();
            labels.push(if display.is_empty() {
                parsed.label
            } else {
                display.to_string()
            });
            inputs.push(parsed.input);
        }

        let key_count = self.config.key_amount as usize;
        self.slots.resize_with(key_count, KeySlot::new);
        self.slots.truncate(key_count);
        self.pressed.resize(key_count, false);
        self.pressed.truncate(key_count);

        self.squares = layout::create_squares(
            self.config.key_amount,
            self.config.outline_thickness,
            self.config.key_size,
            self.config.margin,
            self.config.window_width,
            ratio_y,
        );
        self.labels = labels;
        self.inputs = inputs;
        self.renderer = Some(
            Renderer::new(
                layout::char_size(self.config.key_size),
                layout::text_origin_y(self.config.key_size),
                self.config.window_width,
                ratio_y,
                self.config.fading,
                self.config.background_color,
                &self.config.background_image,
                &self.executable_dir.join("Resources"),
            )
            .map_err(StartupError::Config)?,
        );

        self.period = if self.config.max_fps == 0 {
            Duration::ZERO
        } else {
            Duration::from_secs_f32(1.0 / self.config.max_fps as f32)
        };

        if let Some(window) = &self.window {
            let _ = window.request_inner_size(PhysicalSize::new(
                self.config.window_width,
                self.config.window_height,
            ));
            window.set_window_level(if self.config.always_on_top {
                WindowLevel::AlwaysOnTop
            } else {
                WindowLevel::Normal
            });
            // A transparent overlay is meant to be frameless.
            window.set_decorations(!self.config.transparent_background);
        }
        if let Some(presenter) = self.presenter.as_mut() {
            presenter.set_topmost(self.config.always_on_top);
            presenter
                .set_click_through(self.config.click_through)
                .map_err(StartupError::Config)?;
            presenter
                .set_layered(self.config.transparent_background)
                .map_err(StartupError::Config)?;
        }
        Ok(())
    }

    /// One frame: poll the keys, advance the bars, redraw the pixmap.
    fn step(&mut self) {
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32();
        self.last_frame = now;

        for (index, &input) in self.inputs.iter().enumerate() {
            self.pressed[index] = self.input.is_down(input);
        }
        state::poll(&mut self.slots, &self.pressed);
        state::move_bars(
            &mut self.slots,
            &self.squares,
            self.config.outline_thickness,
            dt,
            self.config.bar_speed,
        );

        if let (Some(renderer), Some(pixmap)) = (self.renderer.as_mut(), self.pixmap.as_mut()) {
            let scene = scene(&self.config, &self.squares, &self.labels, &self.slots);
            renderer.draw_frame(&mut self.text, pixmap, &scene);
        }
    }

    fn present(&mut self) {
        let (Some(presenter), Some(pixmap)) = (self.presenter.as_mut(), self.pixmap.as_ref())
        else {
            return;
        };
        let _ = presenter.present(pixmap);
    }

    fn create_window(&mut self, event_loop: &ActiveEventLoop) -> Result<(), String> {
        let attributes = Window::default_attributes()
            .with_title("KeyOverlay")
            .with_inner_size(PhysicalSize::new(
                self.config.window_width,
                self.config.window_height,
            ))
            .with_resizable(true)
            .with_decorations(!self.config.transparent_background)
            .with_window_level(if self.config.always_on_top {
                WindowLevel::AlwaysOnTop
            } else {
                WindowLevel::Normal
            });

        let window = Arc::new(
            event_loop
                .create_window(attributes)
                .map_err(|error| format!("Could not create the window: {error}"))?,
        );
        let size = window.inner_size();
        let mut presenter = Presenter::new(&window, (size.width, size.height))?;
        presenter.set_topmost(self.config.always_on_top);
        presenter.set_click_through(self.config.click_through)?;
        presenter.set_layered(self.config.transparent_background)?;

        self.pixmap = Pixmap::new(size.width, size.height);
        self.window = Some(window);
        self.presenter = Some(presenter);
        Ok(())
    }
}

struct App {
    executable_dir: PathBuf,
    config_name: String,
    overlay: Overlay,
    settings: Settings,
    settings_window: Option<Arc<Window>>,
    settings_context: Option<softbuffer::Context<Arc<Window>>>,
    settings_surface: Option<softbuffer::Surface<Arc<Window>, Arc<Window>>>,
    settings_pixmap: Option<Pixmap>,
    settings_open: bool,
    /// Scale the settings window was last laid out with.
    settings_scale: f32,
}

impl App {
    fn build(executable_dir: &Path, config_name: &str) -> Result<Self, StartupError> {
        let fonts = Fonts::load();
        let mut overlay = Overlay::new(executable_dir, config_name, fonts.clone())?;
        // Without a CJK face the Chinese labels would render as nothing, so fall
        // back to English rather than showing an unreadable window.
        if fonts.cjk.is_none() && overlay.config.language == Language::Zh {
            overlay.config.language = Language::En;
        }
        let mut settings = Settings::new(fonts).map_err(StartupError::Config)?;
        settings.rescan(&executable_dir.join("Resources"));
        Ok(Self {
            executable_dir: executable_dir.to_path_buf(),
            config_name: config_name.to_string(),
            overlay,
            settings,
            settings_window: None,
            settings_context: None,
            settings_surface: None,
            settings_pixmap: None,
            settings_open: false,
            settings_scale: 0.0,
        })
    }

    fn fatal(&self, message: &str) -> ! {
        report(&self.executable_dir.join("errorMessage.txt"), message)
    }

    /// Creates the settings window on first use, then shows it.
    fn open_settings(&mut self, event_loop: &ActiveEventLoop) {
        if self.settings_window.is_none() {
            // Themed before the window exists: assume the primary monitor.
            let monitor_scale = event_loop
                .primary_monitor()
                .map(|monitor| monitor.scale_factor() as f32)
                .unwrap_or(1.0);
            let scale = ui_scale(&self.overlay.config, monitor_scale);
            let limit = work_area_height().unwrap_or(u32::MAX);
            let wanted_height = (SETTINGS_SIZE.1 as f32 * scale) as u32;
            let attributes = Window::default_attributes()
                .with_title(lang::text(self.overlay.config.language).title)
                .with_inner_size(PhysicalSize::new(
                    (SETTINGS_SIZE.0 as f32 * scale) as u32,
                    wanted_height.min(limit),
                ))
                .with_min_inner_size(PhysicalSize::new(
                    (760.0 * scale) as u32,
                    (560.0 * scale) as u32,
                ))
                .with_resizable(true)
                .with_visible(false);

            let window = match event_loop.create_window(attributes) {
                Ok(window) => Arc::new(window),
                Err(error) => self.fatal(&format!("Could not create the settings window: {error}")),
            };
            let context = match softbuffer::Context::new(window.clone()) {
                Ok(context) => context,
                Err(error) => {
                    self.fatal(&format!("Could not create the graphics context: {error}"))
                }
            };
            let mut surface = match softbuffer::Surface::new(&context, window.clone()) {
                Ok(surface) => surface,
                Err(error) => self.fatal(&format!("Could not create the drawing surface: {error}")),
            };
            let size = window.inner_size();
            if let (Some(width), Some(height)) =
                (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
            {
                let _ = surface.resize(width, height);
            }
            self.settings_pixmap = Pixmap::new(size.width, size.height);
            self.settings.rescan(&self.executable_dir.join("Resources"));
            self.settings_scale = ui_scale(&self.overlay.config, window.scale_factor() as f32);
            self.settings.set_scale(self.settings_scale);
            self.settings_window = Some(window);
            self.settings_context = Some(context);
            self.settings_surface = Some(surface);
        }

        self.settings_open = true;
        if let Some(window) = &self.settings_window {
            window.set_visible(true);
            window.focus_window();
        }
    }

    fn close_settings(&mut self) {
        self.settings_open = false;
        if let Some(window) = &self.settings_window {
            window.set_visible(false);
        }
    }

    /// Rebuilds the overlay from the current configuration.
    fn apply_config(&mut self) {
        if let Err(error) = self.overlay.rebuild() {
            match error {
                StartupError::Config(message) | StartupError::InvalidKey(message) => {
                    self.settings.status(message);
                }
            }
        }
    }

    fn draw_settings(&mut self) {
        if !self.settings_open {
            return;
        }
        let Some(window) = self.settings_window.clone() else {
            return;
        };

        // Follow the display the window currently sits on, and the user's zoom.
        let scale = ui_scale(&self.overlay.config, window.scale_factor() as f32);
        let scale_changed = (scale - self.settings_scale).abs() > f32::EPSILON;
        self.settings_scale = scale;
        self.settings.set_scale(scale);

        let language_before = self.overlay.config.language;
        let outcome = {
            let mut config = self.overlay.config.clone();
            let preview = self.overlay.pixmap.as_ref();
            let Some(pixmap) = self.settings_pixmap.as_mut() else {
                return;
            };
            let outcome = self.settings.draw(
                pixmap,
                &mut config,
                preview,
                hotkey::HOTKEY_LABEL,
                &self.config_name,
            );
            self.overlay.config = config;
            outcome
        };
        if self.overlay.config.language != language_before
            && let Some(window) = &self.settings_window
        {
            window.set_title(lang::text(self.overlay.config.language).title);
        }

        if outcome.changed {
            self.apply_config();
            let t = lang::text(self.overlay.config.language);
            self.settings.status(t.status_applied);
        }
        if outcome.save {
            let message = match config::save(
                &self.executable_dir,
                &self.config_name,
                &self.overlay.config,
            ) {
                Ok(()) => {
                    let t = lang::text(self.overlay.config.language);
                    lang::fill(t.status_saved, &self.config_name, "")
                }
                Err(message) => message,
            };
            self.settings.status(message);
        }
        if outcome.reload {
            match config::load(&self.executable_dir, &self.config_name) {
                Ok(config) => {
                    self.overlay.config = config;
                    self.apply_config();
                    let t = lang::text(self.overlay.config.language);
                    self.settings.status(t.status_reloaded);
                }
                Err(message) => self.settings.status(message),
            }
        }
        if outcome.close {
            self.close_settings();
            return;
        }

        // Fit the window to its content: when the sections need more room, and
        // after a scale change, which resizes everything at once.
        let size = window.inner_size();
        let required_width = (SETTINGS_SIZE.0 as f32 * scale) as u32;
        if outcome.needed_height > size.height as f32 || scale_changed {
            // Never taller than the work area: the content clips instead of
            // running off the bottom of the screen.
            let limit = work_area_height().unwrap_or(u32::MAX);
            let height = (outcome.needed_height.ceil() as u32).min(limit);
            let width = size.width.max(required_width);
            let _ = window.request_inner_size(PhysicalSize::new(width, height));
            return;
        }

        // Blit the finished frame to the settings window.
        let current = self
            .settings_pixmap
            .as_ref()
            .map(|pixmap| (pixmap.width(), pixmap.height()));
        if current != Some((size.width, size.height)) {
            if let (Some(width), Some(height)) =
                (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
            {
                if let Some(surface) = self.settings_surface.as_mut() {
                    let _ = surface.resize(width, height);
                }
                self.settings_pixmap = Pixmap::new(size.width, size.height);
            }
            return;
        }
        if let (Some(surface), Some(pixmap)) = (
            self.settings_surface.as_mut(),
            self.settings_pixmap.as_ref(),
        ) && let Ok(mut buffer) = surface.buffer_mut()
        {
            let source = pixmap.data();
            for (destination, pixel) in buffer.iter_mut().zip(source.as_chunks::<4>().0) {
                *destination = (pixel[0] as u32) << 16 | (pixel[1] as u32) << 8 | pixel[2] as u32;
            }
            let _ = buffer.present();
        }
    }

    /// Keyboard input for the settings window: a focused text field takes the
    /// characters, and Escape leaves the field before it closes the window.
    fn settings_key(&mut self, event: &winit::event::KeyEvent) {
        use winit::keyboard::{Key, NamedKey};

        let editing = self.settings.is_editing();
        match &event.logical_key {
            Key::Named(NamedKey::Escape) => {
                if editing {
                    self.settings.cancel_edit(&self.overlay.config);
                } else {
                    self.close_settings();
                }
            }
            Key::Named(NamedKey::Enter) if editing => {
                if self.settings.commit_edit(&mut self.overlay.config) {
                    self.apply_config();
                }
            }
            Key::Named(NamedKey::Backspace) if editing => self.settings.edit_backspace(),
            _ if editing => {
                if let Some(text) = &event.text {
                    for ch in text.chars() {
                        self.settings.edit_char(ch);
                    }
                }
            }
            _ => {}
        }
    }

    fn tick(&mut self) {
        if self.settings.capture_tick(&mut self.overlay.config) {
            self.apply_config();
        }
        self.overlay.step();
        self.overlay.present();
        self.draw_settings();
    }
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.overlay.window.is_some() {
            return;
        }
        if let Err(message) = self.overlay.create_window(event_loop) {
            self.fatal(&message);
        }
        if let Some(window) = &self.overlay.window {
            window.request_redraw();
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::ToggleSettings => {
                if self.settings_open {
                    self.close_settings();
                } else {
                    self.open_settings(event_loop);
                }
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let overlay_id = self.overlay.window.as_ref().map(|window| window.id());
        let settings_id = self.settings_window.as_ref().map(|window| window.id());

        if Some(window_id) == overlay_id {
            match event {
                WindowEvent::CloseRequested => event_loop.exit(),
                WindowEvent::Resized(size) => {
                    self.overlay.pixmap = Pixmap::new(size.width, size.height);
                    if let Some(presenter) = self.overlay.presenter.as_mut() {
                        presenter.resize(size.width, size.height);
                    }
                }
                WindowEvent::RedrawRequested => self.overlay.present(),
                // Clicking the overlay is the mouse way into the settings; a
                // click-through window never receives this.
                WindowEvent::MouseInput {
                    state: ElementState::Pressed,
                    button: MouseButton::Left,
                    ..
                } => self.open_settings(event_loop),
                _ => {}
            }
            return;
        }

        if Some(window_id) == settings_id {
            match event {
                WindowEvent::CloseRequested => self.close_settings(),
                WindowEvent::Resized(size) => {
                    if let (Some(surface), Some(width), Some(height)) = (
                        self.settings_surface.as_mut(),
                        NonZeroU32::new(size.width),
                        NonZeroU32::new(size.height),
                    ) {
                        let _ = surface.resize(width, height);
                    }
                    self.settings_pixmap = Pixmap::new(size.width, size.height);
                }
                WindowEvent::RedrawRequested => self.draw_settings(),
                WindowEvent::CursorMoved { position, .. } => {
                    self.settings
                        .on_cursor(position.x as f32, position.y as f32);
                }
                WindowEvent::MouseInput {
                    state,
                    button: MouseButton::Left,
                    ..
                } => {
                    let cursor = self.settings.cursor();
                    match state {
                        ElementState::Pressed => self.settings.on_press(cursor.0, cursor.1),
                        ElementState::Released => self.settings.on_release(cursor.0, cursor.1),
                    }
                }
                WindowEvent::KeyboardInput { event, .. }
                    if event.state == ElementState::Pressed =>
                {
                    self.settings_key(&event);
                }
                _ => {}
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let period = self.overlay.period;
        if period.is_zero() {
            self.tick();
            event_loop.set_control_flow(ControlFlow::Poll);
            return;
        }

        let now = Instant::now();
        if now >= self.overlay.next_frame {
            self.tick();
            // Same pacing as SetFramerateLimit: keep the frame interval, but
            // never try to catch up on frames that were already missed.
            self.overlay.next_frame += period;
            if self.overlay.next_frame <= now {
                self.overlay.next_frame = now + period;
            }
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.overlay.next_frame));
    }
}

fn main() {
    let executable_dir = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."));
    let config_name = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "config.txt".to_string());

    let mut app = match App::build(&executable_dir, &config_name) {
        Ok(app) => app,
        Err(StartupError::Config(message)) => {
            report(&executable_dir.join("errorMessage.txt"), &message)
        }
        Err(StartupError::InvalidKey(message)) => {
            report(&executable_dir.join("keyErrorMessage.txt"), &message)
        }
    };

    let event_loop = match EventLoop::<UserEvent>::with_user_event().build() {
        Ok(event_loop) => event_loop,
        Err(error) => app.fatal(&format!("Could not start the event loop: {error}")),
    };
    if let Err(message) = hotkey::spawn(event_loop.create_proxy()) {
        eprintln!("{message}");
        app.settings.status(message);
    }
    if let Err(error) = event_loop.run_app(&mut app) {
        app.fatal(&format!("Event loop error: {error}"));
    }
}

fn scene<'a>(
    config: &'a Config,
    squares: &'a [Square],
    labels: &'a [String],
    slots: &'a [KeySlot],
) -> Scene<'a> {
    Scene {
        squares,
        labels,
        slots,
        background_color: config.background_color,
        key_color: config.key_color,
        bar_color: config.bar_color,
        outline_color: config.border_color,
        font_color: config.font_color,
        press_font_color: config.press_font_color,
        counter: config.key_counter,
        opaque_background: !config.transparent_background,
    }
}

fn report(path: &Path, message: &str) -> ! {
    let _ = std::fs::write(path, message);
    eprintln!("{message}");
    std::process::exit(1);
}
