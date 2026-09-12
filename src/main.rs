//! KeyOverlay - Rust rewrite of the SFML/.NET original.
//!
//! Presets in `presets/*.toml` drive everything. A second window edits the
//! selected preset; it opens with `Ctrl+Alt+K` or by clicking the overlay.
//! Start-up problems are reported through the same files the original used:
//! `errorMessage.txt` for anything, `keyErrorMessage.txt` for invalid key
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
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, Ime, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Icon, Window, WindowId, WindowLevel};

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
/// The application icon: also compiled into the executable's resources, so the
/// file, the taskbar entry and the title bars all show the same mark.
const ICON: &[u8] = include_bytes!("../assets/icon.png");
const SETTINGS_SIZE: (u32, u32) = (980, 780);
/// Base size bump of the settings window: at 100% DPI the original numbers came
/// out smaller than a normal Windows dialog, so everything is drawn 1.25x and
/// then multiplied by the display's own scale factor. `ui_scale` in
/// `presets/default.toml` zooms further, relative to the display, so it
/// survives a monitor change.
const UI_BASE_SCALE: f32 = 1.1;

fn ui_scale(config: &Config, monitor_scale: f32) -> f32 {
    (UI_BASE_SCALE * monitor_scale * config.ui_scale).clamp(0.5, 4.0)
}

/// The primary monitor's work area, so the settings window stays on screen: it
/// is centered when created and never grows past the desktop.
fn work_area() -> Option<RECT> {
    let mut rect: RECT = unsafe { std::mem::zeroed() };
    let ok = unsafe { SystemParametersInfoW(SPI_GETWORKAREA, 0, (&raw mut rect).cast(), 0) };
    (ok != 0).then_some(rect)
}

fn work_area_size() -> (u32, u32) {
    work_area()
        .map(|rect| {
            (
                (rect.right - rect.left).max(0) as u32,
                (rect.bottom - rect.top).max(0) as u32,
            )
        })
        .unwrap_or((u32::MAX, u32::MAX))
}

/// Puts a freshly created window in the middle of the work area.
fn center_on_screen(window: &Window) {
    let Some(work) = work_area() else {
        return;
    };
    let size = window.outer_size();
    let x = work.left + ((work.right - work.left - size.width as i32) / 2).max(0);
    let y = work.top + ((work.bottom - work.top - size.height as i32) / 2).max(0);
    window.set_outer_position(PhysicalPosition::new(x, y));
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
    fn new(executable_dir: &Path, preset_name: &str, fonts: Fonts) -> Result<Self, StartupError> {
        let config = config::load(executable_dir, preset_name).map_err(StartupError::Config)?;
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

        let key_count = self.config.keys.len();
        self.slots.resize_with(key_count, KeySlot::new);
        self.slots.truncate(key_count);
        self.pressed.resize(key_count, false);
        self.pressed.truncate(key_count);

        self.squares = layout::create_squares(
            self.config.keys.len() as u32,
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
                self.config.background_mode,
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
            window.set_window_level(if self.config.always_on_top {
                WindowLevel::AlwaysOnTop
            } else {
                WindowLevel::Normal
            });
            // The overlay owns its drag behavior, so it never needs the
            // native frame. Keeping it frameless from creation prevents a
            // transparent-mode toggle from changing the client origin.
            let _ = window.request_inner_size(PhysicalSize::new(
                self.config.window_width,
                self.config.window_height,
            ));
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
            .with_window_icon(window_icon())
            .with_inner_size(PhysicalSize::new(
                self.config.window_width,
                self.config.window_height,
            ))
            .with_resizable(true)
            .with_decorations(false)
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
    preset_name: String,
    preset_dirty: bool,
    overlay: Overlay,
    settings: Settings,
    settings_window: Option<Arc<Window>>,
    settings_context: Option<softbuffer::Context<Arc<Window>>>,
    settings_surface: Option<softbuffer::Surface<Arc<Window>, Arc<Window>>>,
    settings_pixmap: Option<Pixmap>,
    settings_open: bool,
    /// Scale the settings window was last laid out with.
    settings_scale: f32,
    /// Where the left button went down on the overlay, and where the cursor is,
    /// so a press can become either a drag or a click.
    overlay_press: Option<(f64, f64)>,
    overlay_cursor: (f64, f64),
}

impl App {
    fn build(executable_dir: &Path, preset_name: &str) -> Result<Self, StartupError> {
        let fonts = Fonts::load();
        config::ensure_presets(executable_dir).map_err(StartupError::Config)?;
        let presets = config::list_presets(executable_dir).map_err(StartupError::Config)?;
        let mut overlay = Overlay::new(executable_dir, preset_name, fonts.clone())?;
        // Without a CJK face the Chinese labels would render as nothing, so fall
        // back to English rather than showing an unreadable window.
        if fonts.cjk.is_none() && overlay.config.language == Language::Zh {
            overlay.config.language = Language::En;
        }
        let mut settings = Settings::new(fonts).map_err(StartupError::Config)?;
        settings.rescan(&executable_dir.join("Resources"));
        settings.set_presets(presets);
        Ok(Self {
            executable_dir: executable_dir.to_path_buf(),
            preset_name: preset_name.to_string(),
            preset_dirty: false,
            overlay,
            settings,
            settings_window: None,
            settings_context: None,
            settings_surface: None,
            settings_pixmap: None,
            settings_open: false,
            settings_scale: 0.0,
            overlay_press: None,
            overlay_cursor: (0.0, 0.0),
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
            let (limit_width, limit_height) = work_area_size();
            let wanted_width = ((SETTINGS_SIZE.0 as f32 * scale) as u32).min(limit_width);
            let wanted_height = ((SETTINGS_SIZE.1 as f32 * scale) as u32).min(limit_height);
            let attributes = Window::default_attributes()
                .with_title(lang::text(self.overlay.config.language).title)
                .with_window_icon(window_icon())
                .with_inner_size(PhysicalSize::new(wanted_width, wanted_height))
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
            // So a key label can be typed with an input method, not just ASCII.
            window.set_ime_allowed(true);
            center_on_screen(&window);
            self.settings_scale = ui_scale(&self.overlay.config, window.scale_factor() as f32);
            self.settings.set_scale(self.settings_scale);
            self.settings_window = Some(window);
            self.settings_context = Some(context);
            self.settings_surface = Some(surface);
        }
        self.settings.rescan(&self.executable_dir.join("Resources"));
        self.refresh_presets();

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

    fn refresh_presets(&mut self) {
        match config::list_presets(&self.executable_dir) {
            Ok(presets) => self.settings.set_presets(presets),
            Err(message) => self.settings.status(message),
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
    fn save_current_preset(&mut self) -> Result<(), String> {
        config::save(
            &self.executable_dir,
            &self.preset_name,
            &self.overlay.config,
        )?;
        self.preset_dirty = false;
        Ok(())
    }

    fn load_preset(&mut self, preset_name: &str) -> Result<(), String> {
        let config = config::load(&self.executable_dir, preset_name)?;
        self.preset_name = preset_name.to_string();
        self.overlay.config = config;
        self.preset_dirty = false;
        self.apply_config();
        if let Some(window) = &self.settings_window {
            window.set_title(lang::text(self.overlay.config.language).title);
        }
        Ok(())
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
                &self.preset_name,
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
            self.preset_dirty = true;
            self.apply_config();
            let t = lang::text(self.overlay.config.language);
            self.settings.status(t.status_applied);
        }
        if outcome.save_preset {
            let message = match self.save_current_preset() {
                Ok(()) => lang::text(self.overlay.config.language)
                    .status_saved
                    .to_string(),
                Err(message) => message,
            };
            self.settings.status(message);
        }
        if outcome.reload {
            let current_preset = self.preset_name.clone();
            match self.load_preset(&current_preset) {
                Ok(()) => {
                    let t = lang::text(self.overlay.config.language);
                    self.settings.status(t.status_reloaded);
                }
                Err(message) => self.settings.status(message),
            }
        }
        if let Some(preset_name) = outcome.load_preset
            && preset_name != self.preset_name
        {
            if self.preset_dirty {
                self.settings.prompt_switch(preset_name);
            } else {
                match self.load_preset(&preset_name) {
                    Ok(()) => {
                        let t = lang::text(self.overlay.config.language);
                        self.settings
                            .status(format!("{}: {}", t.status_loaded, self.preset_name));
                    }
                    Err(message) => self.settings.status(message),
                }
            }
        }
        if let Some(preset_name) = outcome.save_and_switch {
            match self.save_current_preset() {
                Ok(()) => match self.load_preset(&preset_name) {
                    Ok(()) => {
                        let t = lang::text(self.overlay.config.language);
                        self.settings
                            .status(format!("{}: {}", t.status_loaded, self.preset_name));
                    }
                    Err(message) => self.settings.status(message),
                },
                Err(message) => {
                    self.settings.prompt_switch(preset_name);
                    self.settings.status(message);
                }
            }
        }
        if let Some(preset_name) = outcome.discard_and_switch {
            match self.load_preset(&preset_name) {
                Ok(()) => {
                    let t = lang::text(self.overlay.config.language);
                    self.settings
                        .status(format!("{}: {}", t.status_loaded, self.preset_name));
                }
                Err(message) => self.settings.status(message),
            }
        }
        if let Some(name) = outcome.create_preset {
            match config::create(&self.executable_dir, &name, &self.overlay.config) {
                Ok(preset_name) => {
                    self.preset_name = preset_name.clone();
                    self.preset_dirty = false;
                    self.settings.close_new_preset();
                    self.refresh_presets();
                    let t = lang::text(self.overlay.config.language);
                    self.settings
                        .status(t.status_created.replacen("{}", &preset_name, 1));
                }
                Err(message) => self.settings.status(message),
            }
        }
        if outcome.delete_preset {
            if self.preset_dirty {
                self.settings
                    .status(lang::text(self.overlay.config.language).status_unsaved_delete);
            } else {
                let deleted_preset = self.preset_name.clone();
                match config::delete(&self.executable_dir, &deleted_preset) {
                    Ok(()) => match self.load_preset(config::DEFAULT_PRESET) {
                        Ok(()) => {
                            self.refresh_presets();
                            let t = lang::text(self.overlay.config.language);
                            self.settings.status(t.status_deleted.replacen(
                                "{}",
                                &deleted_preset,
                                1,
                            ));
                        }
                        Err(message) => self.settings.status(message),
                    },
                    Err(message) => self.settings.status(message),
                }
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
            // Never larger than the work area: the content clips instead of
            // running off the edge of the screen.
            let (limit_width, limit_height) = work_area_size();
            let height = (outcome.needed_height.ceil() as u32).min(limit_height);
            let width = size.width.max(required_width).min(limit_width);
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

        let editing = self.settings.ui_mut().is_editing();
        match &event.logical_key {
            Key::Named(NamedKey::Escape) => {
                if editing {
                    self.settings.ui_mut().cancel_edit();
                } else if !self.settings.cancel_switch_prompt() {
                    self.close_settings();
                }
            }
            Key::Named(NamedKey::Enter) if editing => self.settings.ui_mut().end_edit(),
            Key::Named(NamedKey::Backspace) if editing => self.settings.ui_mut().backspace(),
            _ if editing => {
                if let Some(text) = &event.text {
                    let ui = self.settings.ui_mut();
                    for ch in text.chars() {
                        ui.type_char(ch);
                    }
                }
            }
            _ => {}
        }
    }

    fn tick(&mut self) {
        if self.settings.capture_tick(&mut self.overlay.config) {
            self.preset_dirty = true;
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
                // The whole overlay is a drag handle, because a transparent one
                // has no title bar at all and a 240px wide window is fiddly to
                // grab by a 30px strip. Pressing and moving drags the window,
                // pressing and letting go opens the settings. A click-through
                // window never receives any of this.
                WindowEvent::CursorMoved { position, .. } => {
                    self.overlay_cursor = (position.x, position.y);
                    if let Some((x, y)) = self.overlay_press
                        && (position.x - x).abs() + (position.y - y).abs() > 4.0
                        && let Some(window) = &self.overlay.window
                    {
                        self.overlay_press = None;
                        let _ = window.drag_window();
                    }
                }
                WindowEvent::MouseInput {
                    state: ElementState::Pressed,
                    button: MouseButton::Left,
                    ..
                } => self.overlay_press = Some(self.overlay_cursor),
                WindowEvent::MouseInput {
                    state: ElementState::Released,
                    button: MouseButton::Left,
                    ..
                } if self.overlay_press.is_some() => {
                    self.overlay_press = None;
                    self.open_settings(event_loop);
                }
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
                // What an IME (a Chinese input method, say) committed. Input
                // methods deliver here rather than as `KeyboardInput.text`.
                WindowEvent::Ime(Ime::Commit(text)) => {
                    let ui = self.settings.ui_mut();
                    for ch in text.chars() {
                        ui.type_char(ch);
                    }
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
    let preset_name = std::env::args()
        .nth(1)
        .map(|name| {
            if name == "config.toml" {
                config::DEFAULT_PRESET.to_string()
            } else {
                name
            }
        })
        .unwrap_or_else(|| config::DEFAULT_PRESET.to_string());

    let mut app = match App::build(&executable_dir, &preset_name) {
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

/// Decoded once per window; `winit` takes it as RGBA pixels.
fn window_icon() -> Option<Icon> {
    let image = image::load_from_memory(ICON).ok()?.to_rgba8();
    let (width, height) = image.dimensions();
    Icon::from_rgba(image.into_raw(), width, height).ok()
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
