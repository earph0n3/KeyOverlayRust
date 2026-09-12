//! Settings window: an immediate-mode panel over the live configuration.
//!
//! Edits are applied to the running configuration immediately (`changed`); the
//! preview pane mirrors the overlay, and `Save` writes `config.toml` back.
//! Labels match the configuration names while the interface itself supports
//! English and Chinese.

use std::path::Path;
use std::time::Instant;

use tiny_skia::{
    BlendMode, FilterQuality, Paint, PathBuilder, Pixmap, PixmapPaint, Stroke, Transform,
};

use crate::Fonts;
use crate::config::{BackgroundMode, Color, Config};
use crate::input;
use crate::keys;
use crate::lang;
use crate::render::fill_rect;
use crate::text::TextPainter;
use crate::ui::{self, Rect, Ui};

pub struct Outcome {
    pub changed: bool,
    pub save: bool,
    pub reload: bool,
    pub close: bool,
    /// Height the right column needs, so the window can grow to fit.
    pub needed_height: f32,
}

impl Outcome {
    fn new() -> Self {
        Self {
            changed: false,
            save: false,
            reload: false,
            close: false,
            needed_height: 0.0,
        }
    }
}

pub struct Settings {
    ui: Ui,
    painter: TextPainter,
    color_tab: usize,
    /// Effective UI scale (display DPI times the user's zoom).
    scale: f32,

    capture: Option<usize>,
    capture_armed: bool,
    candidates: Vec<(&'static str, u32)>,
    images: Vec<String>,
    background_menu_open: bool,
    background_mode_menu_open: bool,
    status: Option<(String, Instant)>,
}

impl Settings {
    pub fn new(fonts: Fonts) -> Result<Self, String> {
        Ok(Self {
            ui: Ui::default(),
            painter: fonts.painter()?,
            scale: 1.0,

            color_tab: 0,
            capture: None,
            capture_armed: true,
            candidates: keys::capture_candidates(),
            images: Vec::new(),
            background_menu_open: false,
            background_mode_menu_open: false,
            status: None,
        })
    }

    /// Images offered for `background_image`, taken from `Resources/`.
    pub fn rescan(&mut self, resources_dir: &Path) {
        self.images.clear();
        self.images.push(String::new());
        if let Ok(entries) = std::fs::read_dir(resources_dir) {
            let mut names: Vec<String> = entries
                .filter_map(|entry| entry.ok())
                .map(|entry| entry.file_name().to_string_lossy().to_string())
                .filter(|name| {
                    let lower = name.to_ascii_lowercase();
                    [".png", ".jpg", ".jpeg"]
                        .iter()
                        .any(|extension| lower.ends_with(extension))
                })
                .collect();
            names.sort();
            self.images.extend(names);
        }
    }

    pub fn set_scale(&mut self, scale: f32) {
        self.scale = scale.clamp(0.5, 4.0);
    }

    /// The widget layer owns the numeric fields, so keyboard input goes there.
    pub fn ui_mut(&mut self) -> &mut Ui {
        &mut self.ui
    }

    pub fn on_cursor(&mut self, x: f32, y: f32) {
        self.ui.mouse = (x, y);
    }

    pub fn cursor(&self) -> (f32, f32) {
        self.ui.mouse
    }

    pub fn on_press(&mut self, x: f32, y: f32) {
        self.ui.press((x, y));
    }

    pub fn on_release(&mut self, x: f32, y: f32) {
        self.ui.release((x, y));
    }

    pub fn status(&mut self, text: impl Into<String>) {
        self.status = Some((text.into(), Instant::now()));
    }

    /// While "press a key" is armed, binds the first input that goes down.
    /// Returns true when the configuration changed.
    pub fn capture_tick(&mut self, config: &mut Config) -> bool {
        let Some(index) = self.capture else {
            return false;
        };
        // A field has the keyboard: that is typing, not choosing a binding.
        if self.ui.is_editing() {
            return false;
        }
        if !self.capture_armed {
            // Wait until the click that armed the capture is released again.
            if !self
                .candidates
                .iter()
                .any(|(_, vkey)| input::vk_down(*vkey))
            {
                self.capture_armed = true;
            }
            return false;
        }
        let Some((name, _)) = self
            .candidates
            .iter()
            .find(|(_, vkey)| input::vk_down(*vkey))
        else {
            return false;
        };
        let name = (*name).to_string();
        if index < config.keys.len() {
            config.keys[index] = name;
        }
        self.capture = None;
        true
    }

    pub fn draw(
        &mut self,
        pm: &mut Pixmap,
        config: &mut Config,
        preview: Option<&Pixmap>,
        hotkey: &str,
    ) -> Outcome {
        let mut outcome = Outcome::new();
        let m = ui::Metrics::new(self.scale);
        self.ui.begin(m);

        let t = lang::text(config.language);
        let width = pm.width() as f32;
        let height = pm.height() as f32;
        let theme = ui::theme();
        let note = t.note.replacen("{}", hotkey, 1);

        ui::panel(pm, Rect::new(0.0, 0.0, width, height), theme.background);
        ui::panel(pm, Rect::new(0.0, 0.0, width, m.px(40.0)), theme.panel);
        ui::label(
            pm,
            t.title,
            Rect::new(m.px(16.0), 0.0, m.px(300.0), m.px(40.0)),
            m.heading,
            theme.text,
            &mut self.painter,
        );
        ui::label(
            pm,
            &note,
            Rect::new(
                m.px(210.0),
                0.0,
                (width - m.px(210.0) - m.px(192.0)).max(m.px(200.0)),
                m.px(40.0),
            ),
            m.small,
            theme.text_dim,
            &mut self.painter,
        );
        // Language switch, so it is reachable from anywhere in the window.
        if ui::button(
            &mut self.ui,
            pm,
            config.language.name(),
            Rect::new(width - m.px(180.0), m.px(6.0), m.px(76.0), m.px(28.0)),
            &mut self.painter,
        ) {
            config.language = config.language.next();
            outcome.changed = true;
        }
        if ui::button(
            &mut self.ui,
            pm,
            t.close,
            Rect::new(width - m.px(92.0), m.px(6.0), m.px(76.0), m.px(28.0)),
            &mut self.painter,
        ) {
            outcome.close = true;
        }

        // The status message sits above the preview, so nothing below it moves
        // while it is showing.
        let status_row = Rect::new(m.px(16.0), m.px(56.0), m.px(268.0), m.px(20.0));
        if let Some((text, at)) = &self.status
            && at.elapsed().as_secs() < 5
        {
            ui::label(
                pm,
                text,
                status_row,
                m.small,
                theme.accent,
                &mut self.painter,
            );
        }

        let preview_rect = Rect::new(
            m.px(16.0),
            m.px(82.0),
            m.px(268.0),
            (height - m.px(224.0)).max(m.px(120.0)),
        );
        ui::panel(pm, preview_rect, theme.panel);
        ui::label(
            pm,
            t.live_preview,
            Rect::new(
                preview_rect.x + m.px(10.0),
                preview_rect.y + m.px(6.0),
                m.px(160.0),
                m.px(18.0),
            ),
            m.small,
            theme.text_dim,
            &mut self.painter,
        );
        self.draw_preview(
            pm,
            Rect::new(
                preview_rect.x + m.px(10.0),
                preview_rect.y + m.px(28.0),
                preview_rect.w - m.px(20.0),
                preview_rect.h - m.px(38.0),
            ),
            preview,
            m,
        );

        // Actions and the interface rows are stacked with a cursor, so a row can
        // never drift into the next one.
        let mut y = preview_rect.bottom() + m.px(14.0);
        if ui::button(
            &mut self.ui,
            pm,
            t.save,
            Rect::new(m.px(16.0), y, m.px(162.0), m.px(30.0)),
            &mut self.painter,
        ) {
            outcome.save = true;
        }
        if ui::button(
            &mut self.ui,
            pm,
            t.reload,
            Rect::new(m.px(186.0), y, m.px(98.0), m.px(30.0)),
            &mut self.painter,
        ) {
            outcome.reload = true;
        }
        y += m.px(38.0);

        ui::label(
            pm,
            t.section_interface,
            Rect::new(m.px(16.0), y, m.px(268.0), m.px(18.0)),
            m.small,
            theme.text_dim,
            &mut self.painter,
        );
        y += m.px(24.0);

        let row = Rect::new(m.px(16.0), y, m.px(268.0), m.row);
        ui::label(
            pm,
            t.ui_scale,
            Rect::new(row.x, row.y, m.px(100.0), row.h),
            m.text,
            theme.text_dim,
            &mut self.painter,
        );
        let field = Rect::new(row.x + m.px(104.0), row.y, m.px(100.0), row.h);
        let shown = format!("{:.2}", config.ui_scale);
        if let Some(typed) = self.ui.field(pm, field, &shown, m, &mut self.painter) {
            let value = typed.clamp(0.5, 4.0);
            if (value - config.ui_scale).abs() > 0.005 {
                config.ui_scale = value;
                outcome.changed = true;
            }
        }
        if ui::button(
            &mut self.ui,
            pm,
            t.reset,
            Rect::new(row.right() - m.px(70.0), row.y, m.px(70.0), row.h),
            &mut self.painter,
        ) {
            config.ui_scale = 1.0;
            outcome.changed = true;
        }
        y = row.bottom() + m.px(6.0);

        ui::label(
            pm,
            t.ui_scale_hint,
            Rect::new(m.px(16.0), y, m.px(268.0), m.px(20.0)),
            m.small,
            theme.text_dim,
            &mut self.painter,
        );

        // Right column: sections.
        let mut y = m.px(52.0);
        let x = m.px(300.0);
        let section_width = (width - x - m.px(16.0)).max(m.px(320.0));

        y = self.keys_section(
            pm,
            Rect::new(x, y, section_width, 0.0),
            config,
            &mut outcome,
            m,
        );
        y = self.layout_section(
            pm,
            Rect::new(x, y, section_width, 0.0),
            config,
            &mut outcome,
            m,
        );
        y = self.appearance_section(
            pm,
            Rect::new(x, y, section_width, 0.0),
            config,
            &mut outcome,
            m,
        );
        y = self.window_section(
            pm,
            Rect::new(x, y, section_width, 0.0),
            config,
            &mut outcome,
            m,
        );
        outcome.needed_height = y + m.px(4.0);

        self.ui.end();
        outcome
    }

    fn draw_preview(
        &mut self,
        pm: &mut Pixmap,
        rect: Rect,
        preview: Option<&Pixmap>,
        m: ui::Metrics,
    ) {
        self.checkerboard(pm, rect, m);

        if let Some(preview) = preview
            && preview.width() > 0
            && preview.height() > 0
        {
            let scale = (rect.w / preview.width() as f32)
                .min(rect.h / preview.height() as f32)
                .min(1.0);
            let draw_width = preview.width() as f32 * scale;
            let draw_height = preview.height() as f32 * scale;
            let target_x = rect.x + (rect.w - draw_width) / 2.0;
            let target_y = rect.y + (rect.h - draw_height) / 2.0;
            let paint = PixmapPaint {
                opacity: 1.0,
                blend_mode: BlendMode::SourceOver,
                quality: FilterQuality::Bilinear,
            };
            // `draw_pixmap` applies `transform` to the destination rectangle as
            // well, so the placement has to live in the transform and the rect
            // origin must stay at (0, 0).
            let transform = Transform::from_translate(target_x, target_y).pre_scale(scale, scale);
            pm.draw_pixmap(0, 0, preview.as_ref(), &paint, transform, None);
        }

        let theme = ui::theme();
        if let Some(border) = tiny_skia::Rect::from_xywh(rect.x, rect.y, rect.w, rect.h) {
            let mut builder = PathBuilder::new();
            builder.push_rect(border);
            if let Some(path) = builder.finish() {
                let mut paint = Paint::default();
                paint.set_color_rgba8(theme.accent.r, theme.accent.g, theme.accent.b, 80);
                paint.anti_alias = false;
                let stroke = Stroke {
                    width: m.px(1.0).max(1.0),
                    ..Default::default()
                };
                pm.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
            }
        }
    }

    /// Alternating squares, so a transparent background is visibly transparent.
    fn checkerboard(&self, pm: &mut Pixmap, rect: Rect, m: ui::Metrics) {
        let cell = m.px(10.0).max(4.0);
        let mut row = 0;
        let mut y = rect.y;
        while y < rect.bottom() {
            let mut column = 0;
            let mut x = rect.x;
            while x < rect.right() {
                let shade = if (row + column) % 2 == 0 { 58 } else { 44 };
                fill_rect(
                    pm,
                    x,
                    y,
                    cell.min(rect.right() - x),
                    cell.min(rect.bottom() - y),
                    Color {
                        r: shade,
                        g: shade,
                        b: shade + 4,
                        a: 255,
                    },
                );
                x += cell;
                column += 1;
            }
            y += cell;
            row += 1;
        }
    }

    fn section(&mut self, pm: &mut Pixmap, rect: Rect, title: &str, m: ui::Metrics) -> Rect {
        let theme = ui::theme();
        ui::panel(pm, rect, theme.panel);
        ui::panel(
            pm,
            Rect::new(rect.x, rect.y, m.px(3.0), rect.h),
            theme.accent,
        );
        ui::label(
            pm,
            title,
            Rect::new(
                rect.x + m.px(12.0),
                rect.y + m.px(4.0),
                rect.w - m.px(24.0),
                m.px(20.0),
            ),
            m.title,
            theme.text,
            &mut self.painter,
        );
        Rect::new(
            rect.x + m.px(12.0),
            rect.y + m.px(30.0),
            rect.w - m.px(24.0),
            rect.h - m.px(38.0),
        )
    }

    fn keys_section(
        &mut self,
        pm: &mut Pixmap,
        rect: Rect,
        config: &mut Config,
        outcome: &mut Outcome,
        m: ui::Metrics,
    ) -> f32 {
        let t = lang::text(config.language);
        let theme = ui::theme();
        let rows = config.keys.len() as f32;
        let rect = Rect::new(
            rect.x,
            rect.y,
            rect.w,
            m.px(40.0) + (rows + 1.0) * (m.row + m.px(4.0)),
        );
        let inner = self.section(pm, rect, t.section_keys, m);
        let mut y = inner.y;

        for index in 0..config.keys.len() {
            let row = row_rect(inner, &mut y, m);
            let capturing = self.capture == Some(index);
            let caption = if capturing {
                if self.capture_armed {
                    t.press_key.to_string()
                } else {
                    t.release_key.to_string()
                }
            } else {
                match keys::parse(&config.keys[index]) {
                    Ok(parsed) => parsed.label,
                    Err(_) => format!("? {}", config.keys[index]),
                }
            };
            if ui::button(
                &mut self.ui,
                pm,
                &caption,
                Rect::new(row.x, row.y, m.px(200.0), m.row),
                &mut self.painter,
            ) {
                if capturing {
                    self.capture = None;
                } else {
                    self.capture = Some(index);
                    self.capture_armed = false;
                }
            }

            let display_rect = Rect::new(
                row.x + m.px(210.0),
                row.y,
                (row.w - m.px(210.0) - m.px(34.0)).max(m.px(60.0)),
                m.row,
            );
            let display = config.display_keys.get(index).cloned().unwrap_or_default();
            if let Some(text) =
                self.ui
                    .text_field(pm, display_rect, &display, t.key_name, m, &mut self.painter)
            {
                if config.display_keys.len() <= index {
                    config.display_keys.resize(index + 1, String::new());
                }
                config.display_keys[index] = text;
                outcome.changed = true;
            }

            if ui::button(
                &mut self.ui,
                pm,
                "x",
                Rect::new(row.right() - m.px(28.0), row.y, m.px(28.0), m.row),
                &mut self.painter,
            ) {
                config.remove_key(index);
                self.capture = None;
                outcome.changed = true;
                break;
            }
        }

        let add_row = row_rect(inner, &mut y, m);
        if ui::button(
            &mut self.ui,
            pm,
            t.add_key,
            Rect::new(add_row.x, add_row.y, m.px(120.0), m.row),
            &mut self.painter,
        ) {
            config.add_key();
            outcome.changed = true;
        }
        ui::label(
            pm,
            t.binding_hint,
            Rect::new(
                add_row.x + m.px(132.0),
                add_row.y,
                (inner.w - m.px(132.0)).max(m.px(60.0)),
                m.row,
            ),
            m.small,
            theme.text_dim,
            &mut self.painter,
        );

        rect.bottom() + m.px(8.0)
    }

    fn layout_section(
        &mut self,
        pm: &mut Pixmap,
        rect: Rect,
        config: &mut Config,
        outcome: &mut Outcome,
        m: ui::Metrics,
    ) -> f32 {
        let t = lang::text(config.language);
        let lines = 4.0;
        let rect = Rect::new(
            rect.x,
            rect.y,
            rect.w,
            m.px(40.0) + lines * (m.row + m.px(4.0)),
        );
        let inner = self.section(pm, rect, t.section_layout, m);
        let mut changed = false;

        changed |= int_slider(
            &mut self.ui,
            pm,
            &mut self.painter,
            t.key_size,
            grid_rect(inner, inner.y, 0, 2, m),
            &mut config.key_size,
            20,
            200,
        );
        changed |= int_slider(
            &mut self.ui,
            pm,
            &mut self.painter,
            t.margin,
            grid_rect(inner, inner.y, 1, 2, m),
            &mut config.margin,
            0,
            200,
        );
        changed |= int_slider(
            &mut self.ui,
            pm,
            &mut self.painter,
            t.outline,
            grid_rect(inner, inner.y, 2, 2, m),
            &mut config.outline_thickness,
            0,
            20,
        );
        changed |= float_slider(
            &mut self.ui,
            pm,
            &mut self.painter,
            t.bar_speed,
            grid_rect(inner, inner.y, 3, 2, m),
            &mut config.bar_speed,
            50.0,
            2000.0,
            10.0,
        );
        changed |= uint_slider(
            &mut self.ui,
            pm,
            &mut self.painter,
            t.window_width,
            grid_rect(inner, inner.y, 4, 2, m),
            &mut config.window_width,
            120,
            1920,
        );
        changed |= uint_slider(
            &mut self.ui,
            pm,
            &mut self.painter,
            t.window_height,
            grid_rect(inner, inner.y, 5, 2, m),
            &mut config.window_height,
            120,
            1080,
        );
        changed |= uint_slider(
            &mut self.ui,
            pm,
            &mut self.painter,
            t.max_fps,
            grid_rect(inner, inner.y, 6, 2, m),
            &mut config.max_fps,
            0,
            240,
        );
        changed |= ui::toggle(
            &mut self.ui,
            pm,
            t.key_counter,
            grid_rect(inner, inner.y, 7, 2, m),
            &mut config.key_counter,
            &mut self.painter,
        );

        outcome.changed |= changed;
        rect.bottom() + m.px(8.0)
    }

    fn appearance_section(
        &mut self,
        pm: &mut Pixmap,
        rect: Rect,
        config: &mut Config,
        outcome: &mut Outcome,
        m: ui::Metrics,
    ) -> f32 {
        let t = lang::text(config.language);
        let rect = Rect::new(
            rect.x,
            rect.y,
            rect.w,
            m.px(40.0) + 3.0 * m.row + 2.0 * (m.row + m.px(4.0)),
        );
        let inner = self.section(pm, rect, t.section_appearance, m);
        let theme = ui::theme();
        let mut y = inner.y;
        let mut changed = false;

        changed |= ui::toggle(
            &mut self.ui,
            pm,
            t.fading,
            row_rect(inner, &mut y, m),
            &mut config.fading,
            &mut self.painter,
        );

        let tab_row = row_rect(inner, &mut y, m);
        let tab_width = tab_row.w / t.color_names.len() as f32;
        for (index, name) in t.color_names.iter().enumerate() {
            let cell = Rect::new(
                tab_row.x + index as f32 * tab_width,
                tab_row.y,
                (tab_width - m.px(4.0)).max(m.px(20.0)),
                tab_row.h,
            );
            let selected = self.color_tab == index;
            ui::panel(
                pm,
                cell,
                if selected {
                    theme.accent
                } else {
                    theme.control
                },
            );
            ui::label_centered(
                pm,
                name,
                cell,
                m.small,
                if selected {
                    theme.background
                } else {
                    theme.text_dim
                },
                &mut self.painter,
            );
            if self.ui.clicked_in(cell) {
                self.color_tab = index;
            }
        }

        let color = color_of(config, self.color_tab);
        let info_row = row_rect(inner, &mut y, m);
        ui::label(
            pm,
            &format!(
                "r,g,b,a = {}, {}, {}, {}",
                color.r, color.g, color.b, color.a
            ),
            Rect::new(info_row.x, info_row.y, info_row.w - m.px(90.0), info_row.h),
            m.small,
            theme.text_dim,
            &mut self.painter,
        );
        fill_rect(
            pm,
            info_row.right() - m.px(84.0),
            info_row.y + m.px(3.0),
            m.px(84.0),
            info_row.h - m.px(6.0),
            Color { a: 255, ..color },
        );

        let mut updated = color;
        let mut color_changed = false;
        for (index, channel) in t.channels.iter().enumerate() {
            let target = match index {
                0 => &mut updated.r,
                1 => &mut updated.g,
                2 => &mut updated.b,
                _ => &mut updated.a,
            };
            color_changed |= channel_slider(
                &mut self.ui,
                pm,
                &mut self.painter,
                channel,
                grid_rect(inner, y, index, 2, m),
                target,
            );
        }
        if color_changed {
            set_color(config, self.color_tab, updated);
            changed = true;
        }

        outcome.changed |= changed;
        rect.bottom() + m.px(8.0)
    }

    fn window_section(
        &mut self,
        pm: &mut Pixmap,
        rect: Rect,
        config: &mut Config,
        outcome: &mut Outcome,
        m: ui::Metrics,
    ) -> f32 {
        let t = lang::text(config.language);
        let missing_image = !config.background_image.is_empty()
            && !self
                .images
                .iter()
                .any(|name| name == &config.background_image);
        let mut image_names = self.images.clone();
        if image_names.is_empty() {
            image_names.push(String::new());
        }
        if missing_image {
            image_names.insert(0, config.background_image.clone());
        }
        let image_menu_rows = if self.background_menu_open {
            image_names.len()
        } else {
            0
        };
        let mode_menu_rows = if self.background_mode_menu_open {
            BackgroundMode::ALL.len()
        } else {
            0
        };
        let rows = 5 + image_menu_rows + mode_menu_rows;
        let rect = Rect::new(rect.x, rect.y, rect.w, m.px(40.0) + rows as f32 * m.row);
        let inner = self.section(pm, rect, t.section_window, m);
        let theme = ui::theme();
        let mut y = inner.y;
        let mut changed = false;

        if ui::toggle(
            &mut self.ui,
            pm,
            t.transparent,
            row_rect(inner, &mut y, m),
            &mut config.transparent_background,
            &mut self.painter,
        ) {
            // A layered window with an opaque colour would look just like the
            // plain one, so the toggle also makes the background see-through.
            // The alpha can still be dialled in by hand for a translucent panel.
            config.background_color.a = if config.transparent_background {
                0
            } else {
                255
            };
            changed = true;
        }
        changed |= ui::toggle(
            &mut self.ui,
            pm,
            t.click_through,
            row_rect(inner, &mut y, m),
            &mut config.click_through,
            &mut self.painter,
        );
        changed |= ui::toggle(
            &mut self.ui,
            pm,
            t.always_on_top,
            row_rect(inner, &mut y, m),
            &mut config.always_on_top,
            &mut self.painter,
        );

        let image_row = row_rect(inner, &mut y, m);
        ui::label(
            pm,
            t.background_image,
            Rect::new(image_row.x, image_row.y, m.px(136.0), image_row.h),
            m.text,
            theme.text_dim,
            &mut self.painter,
        );
        let image_button = Rect::new(
            image_row.x + m.px(136.0),
            image_row.y,
            (image_row.w - m.px(136.0)).max(m.px(80.0)),
            image_row.h,
        );
        let image_label = if config.background_image.is_empty() {
            t.none.to_string()
        } else if missing_image {
            format!("{}{}", config.background_image, t.missing_suffix)
        } else {
            config.background_image.clone()
        };
        if ui::button(
            &mut self.ui,
            pm,
            &image_label,
            image_button,
            &mut self.painter,
        ) {
            self.background_menu_open = !self.background_menu_open;
            self.background_mode_menu_open = false;
        }

        if self.background_menu_open {
            for name in &image_names {
                let item = Rect::new(image_button.x, y, image_button.w, m.row);
                let label = if name.is_empty() {
                    t.none.to_string()
                } else if missing_image && name == &config.background_image {
                    format!("{}{}", name, t.missing_suffix)
                } else {
                    name.clone()
                };
                let selected = name == &config.background_image;
                if ui::list_item(&mut self.ui, pm, &label, item, selected, &mut self.painter) {
                    config.background_image = name.clone();
                    self.background_menu_open = false;
                    changed = true;
                }
                y += m.row;
            }
        }

        let mode_row = row_rect(inner, &mut y, m);
        ui::label(
            pm,
            t.background_mode,
            Rect::new(mode_row.x, mode_row.y, m.px(136.0), mode_row.h),
            m.text,
            theme.text_dim,
            &mut self.painter,
        );
        let mode_button = Rect::new(
            mode_row.x + m.px(136.0),
            mode_row.y,
            (mode_row.w - m.px(136.0)).max(m.px(80.0)),
            mode_row.h,
        );
        let mode_label = t.background_modes[config.background_mode.index()];
        if ui::button(&mut self.ui, pm, mode_label, mode_button, &mut self.painter) {
            self.background_mode_menu_open = !self.background_mode_menu_open;
            self.background_menu_open = false;
        }

        if self.background_mode_menu_open {
            for mode in BackgroundMode::ALL {
                let item = Rect::new(mode_button.x, y, mode_button.w, m.row);
                if ui::list_item(
                    &mut self.ui,
                    pm,
                    t.background_modes[mode.index()],
                    item,
                    mode == config.background_mode,
                    &mut self.painter,
                ) {
                    config.background_mode = mode;
                    self.background_mode_menu_open = false;
                    changed = true;
                }
                y += m.row;
            }
        }

        if changed {
            outcome.changed = true;
        }
        rect.bottom() + m.px(8.0)
    }
}

/// The `index`-th cell of a `columns`-wide grid, whose first line starts at
/// `top` (so grids can follow other rows inside the same section).
fn grid_rect(inner: Rect, top: f32, index: usize, columns: usize, m: ui::Metrics) -> Rect {
    let gap = m.px(14.0);
    let width = (inner.w - gap * (columns - 1) as f32) / columns as f32;
    let column = index % columns;
    let line = index / columns;
    Rect::new(
        inner.x + (width + gap) * column as f32,
        top + (m.row + m.px(4.0)) * line as f32,
        width,
        m.row,
    )
}

fn row_rect(inner: Rect, y: &mut f32, m: ui::Metrics) -> Rect {
    let rect = Rect::new(inner.x, *y, inner.w, m.row);
    *y += m.row;
    rect
}

fn color_of(config: &Config, tab: usize) -> Color {
    match tab {
        0 => config.background_color,
        1 => config.key_color,
        2 => config.border_color,
        3 => config.bar_color,
        4 => config.font_color,
        _ => config.press_font_color,
    }
}

fn set_color(config: &mut Config, tab: usize, color: Color) {
    match tab {
        0 => config.background_color = color,
        1 => config.key_color = color,
        2 => config.border_color = color,
        3 => config.bar_color = color,
        4 => config.font_color = color,
        _ => config.press_font_color = color,
    }
}

#[allow(clippy::too_many_arguments)]
fn int_slider(
    ui: &mut Ui,
    pm: &mut Pixmap,
    painter: &mut TextPainter,
    label: &str,
    rect: Rect,
    value: &mut i32,
    min: i32,
    max: i32,
) -> bool {
    let mut current = *value as f32;
    let changed = ui.slider(
        pm,
        label,
        rect,
        &mut current,
        min as f32,
        max as f32,
        1.0,
        |v| format!("{}", v as i32),
        painter,
    );
    if changed {
        *value = current as i32;
    }
    changed
}

#[allow(clippy::too_many_arguments)]
fn uint_slider(
    ui: &mut Ui,
    pm: &mut Pixmap,
    painter: &mut TextPainter,
    label: &str,
    rect: Rect,
    value: &mut u32,
    min: u32,
    max: u32,
) -> bool {
    let mut current = *value as f32;
    let changed = ui.slider(
        pm,
        label,
        rect,
        &mut current,
        min as f32,
        max as f32,
        1.0,
        |v| format!("{}", v as i64),
        painter,
    );
    if changed {
        *value = current as u32;
    }
    changed
}

#[allow(clippy::too_many_arguments)]
fn float_slider(
    ui: &mut Ui,
    pm: &mut Pixmap,
    painter: &mut TextPainter,
    label: &str,
    rect: Rect,
    value: &mut f32,
    min: f32,
    max: f32,
    step: f32,
) -> bool {
    ui.slider(
        pm,
        label,
        rect,
        value,
        min,
        max,
        step,
        |v| format!("{v}"),
        painter,
    )
}

#[allow(clippy::too_many_arguments)]
fn channel_slider(
    ui: &mut Ui,
    pm: &mut Pixmap,
    painter: &mut TextPainter,
    label: &str,
    rect: Rect,
    value: &mut u8,
) -> bool {
    let mut current = *value as f32;
    let changed = ui.slider(
        pm,
        label,
        rect,
        &mut current,
        0.0,
        255.0,
        1.0,
        |v| format!("{}", v as i32),
        painter,
    );
    if changed {
        *value = current as u8;
    }
    changed
}
