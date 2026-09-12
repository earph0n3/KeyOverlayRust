//! Minimal immediate-mode widget toolkit, drawn with the same tiny-skia pass as
//! the overlay. Only what the settings window needs.

use tiny_skia::Pixmap;

use crate::config::Color;
use crate::render::fill_rect;
use crate::text::TextPainter;

pub const TEXT: u32 = 14;
pub const TEXT_SMALL: u32 = 12;

/// Layout metrics for one draw pass. Everything the settings window draws is
/// expressed in these units, so a single scale factor resizes the whole UI -
/// both for the display's DPI and for the user's own zoom.
#[derive(Clone, Copy, Debug)]
pub struct Metrics {
    pub scale: f32,
    /// Body text size.
    pub text: u32,
    /// Small/label text size.
    pub small: u32,
    /// Section title size.
    pub title: u32,
    /// Window title size.
    pub heading: u32,
    /// Height of one settings row.
    pub row: f32,
}

impl Metrics {
    pub fn new(scale: f32) -> Self {
        Self {
            scale,
            text: (TEXT as f32 * scale).round().max(9.0) as u32,
            small: (TEXT_SMALL as f32 * scale).round().max(8.0) as u32,
            title: (15.0 * scale).round().max(10.0) as u32,
            heading: (17.0 * scale).round().max(11.0) as u32,
            row: (24.0 * scale).round().max(14.0),
        }
    }

    /// Scales a layout length.
    pub fn px(self, value: f32) -> f32 {
        value * self.scale
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    pub fn contains(&self, point: (f32, f32)) -> bool {
        point.0 >= self.x
            && point.0 < self.x + self.w
            && point.1 >= self.y
            && point.1 < self.y + self.h
    }

    pub fn inset(&self, amount: f32) -> Rect {
        Rect::new(
            self.x + amount,
            self.y + amount,
            self.w - amount * 2.0,
            self.h - amount * 2.0,
        )
    }

    pub fn right(&self) -> f32 {
        self.x + self.w
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
}

#[derive(Clone, Copy)]
pub struct Theme {
    pub background: Color,
    pub panel: Color,
    pub row: Color,
    pub control: Color,
    pub control_hot: Color,
    pub accent: Color,
    pub button: Color,
    pub button_hot: Color,
    pub text: Color,
    pub text_dim: Color,
}

pub fn theme() -> Theme {
    Theme {
        background: Color {
            r: 24,
            g: 26,
            b: 32,
            a: 255,
        },
        panel: Color {
            r: 34,
            g: 37,
            b: 45,
            a: 255,
        },
        row: Color {
            r: 43,
            g: 47,
            b: 57,
            a: 255,
        },
        control: Color {
            r: 55,
            g: 60,
            b: 72,
            a: 255,
        },
        control_hot: Color {
            r: 72,
            g: 79,
            b: 94,
            a: 255,
        },
        accent: Color {
            r: 88,
            g: 166,
            b: 255,
            a: 255,
        },
        button: Color {
            r: 62,
            g: 68,
            b: 82,
            a: 255,
        },
        button_hot: Color {
            r: 80,
            g: 88,
            b: 106,
            a: 255,
        },
        text: Color {
            r: 233,
            g: 236,
            b: 242,
            a: 255,
        },
        text_dim: Color {
            r: 158,
            g: 165,
            b: 180,
            a: 255,
        },
    }
}

pub struct Ui {
    /// Layout metrics for the current frame, set by `begin`.
    pub m: Metrics,
    pub mouse: (f32, f32),
    pub pressed: bool,
    pub released: bool,
    pub active: Option<u64>,
    press_origin: Option<(f32, f32)>,
    next_id: u64,
}

impl Default for Ui {
    fn default() -> Self {
        Self {
            m: Metrics::new(1.0),
            mouse: (0.0, 0.0),
            pressed: false,
            released: false,
            active: None,
            press_origin: None,
            next_id: 0,
        }
    }
}

impl Ui {
    /// Called once per frame before drawing.
    pub fn begin(&mut self, m: Metrics) {
        self.m = m;
        self.next_id = 0;
    }

    /// Called once per frame after drawing.
    pub fn end(&mut self) {
        self.released = false;
        if !self.pressed {
            self.press_origin = None;
            self.active = None;
        }
    }

    pub fn press(&mut self, position: (f32, f32)) {
        self.mouse = position;
        self.pressed = true;
        self.press_origin = Some(position);
    }

    pub fn release(&mut self, position: (f32, f32)) {
        self.mouse = position;
        self.pressed = false;
        self.released = true;
    }

    fn id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    /// True when a press inside `rect` was released inside `rect` on this frame.
    pub fn clicked_in(&self, rect: Rect) -> bool {
        self.released
            && self
                .press_origin
                .is_some_and(|origin| rect.contains(origin))
            && rect.contains(self.mouse)
    }

    fn hovered(&self, rect: Rect) -> bool {
        rect.contains(self.mouse)
    }

    /// Horizontal slider over a fixed range, snapped to `step`.
    #[allow(clippy::too_many_arguments)]
    pub fn slider(
        &mut self,
        pm: &mut Pixmap,
        text: &str,
        rect: Rect,
        value: &mut f32,
        min: f32,
        max: f32,
        step: f32,
        display: impl Fn(f32) -> String,
        painter: &mut TextPainter,
    ) -> bool {
        let theme = theme();
        let m = self.m;
        let id = self.id();
        let label_width = m.px(132.0).min(rect.w * 0.4);
        let value_width = m.px(62.0);
        let track = Rect::new(
            rect.x + label_width,
            rect.y + rect.h / 2.0 - m.px(7.0),
            (rect.w - label_width - value_width - m.px(8.0)).max(m.px(40.0)),
            m.px(14.0),
        );

        let mut changed = false;
        if self.pressed
            && track.contains(self.mouse)
            && self
                .press_origin
                .is_some_and(|origin| track.contains(origin))
        {
            self.active = Some(id);
        }
        if self.pressed && self.active == Some(id) {
            let ratio = ((self.mouse.0 - track.x) / track.w).clamp(0.0, 1.0);
            let next = (((min + ratio * (max - min)) / step).round() * step).clamp(min, max);
            if (next - *value).abs() > f32::EPSILON {
                *value = next;
                changed = true;
            }
        }

        let height = painter.measure(text, m.text).1;
        painter.draw_at(
            pm,
            text,
            rect.x,
            rect.y + (rect.h - height) / 2.0,
            m.text,
            theme.text_dim,
        );

        panel(
            pm,
            Rect::new(track.x, track.y + m.px(4.0), track.w, m.px(6.0)),
            theme.control,
        );
        let ratio = if (max - min).abs() < f32::EPSILON {
            0.0
        } else {
            ((*value - min) / (max - min)).clamp(0.0, 1.0)
        };
        panel(
            pm,
            Rect::new(track.x, track.y + m.px(4.0), track.w * ratio, m.px(6.0)),
            theme.accent,
        );
        let knob = Rect::new(
            track.x + track.w * ratio - m.px(4.0),
            track.y,
            m.px(8.0),
            m.px(14.0),
        );
        panel(
            pm,
            knob,
            if self.hovered(track) || self.active == Some(id) {
                theme.control_hot
            } else {
                theme.control
            },
        );

        let shown = display(*value);
        let shown_height = painter.measure(&shown, m.small).1;
        painter.draw_at(
            pm,
            &shown,
            track.right() + m.px(8.0),
            rect.y + (rect.h - shown_height) / 2.0,
            m.small,
            theme.text,
        );

        changed
    }
}

pub fn panel(pm: &mut Pixmap, rect: Rect, color: Color) {
    fill_rect(pm, rect.x, rect.y, rect.w, rect.h, color);
}

pub fn label(
    pm: &mut Pixmap,
    text: &str,
    rect: Rect,
    size: u32,
    color: Color,
    painter: &mut TextPainter,
) {
    let (width, height) = painter.measure(text, size);
    warn_if_overflow(text, width, rect);
    painter.draw_at(
        pm,
        text,
        rect.x,
        rect.y + (rect.h - height) / 2.0,
        size,
        color,
    );
}

/// Text is never clipped, so anything wider than its rect draws over whatever
/// comes next. Debug builds report that on stderr; release builds pay nothing.
fn warn_if_overflow(text: &str, width: f32, rect: Rect) {
    if cfg!(debug_assertions) && width > rect.w + 0.5 {
        eprintln!(
            "layout: {text:?} is {width:.0}px wide but its rect is {:.0}px",
            rect.w
        );
    }
}

pub fn label_centered(
    pm: &mut Pixmap,
    text: &str,
    rect: Rect,
    size: u32,
    color: Color,
    painter: &mut TextPainter,
) {
    let (width, _) = painter.measure(text, size);
    warn_if_overflow(text, width, rect);
    painter.draw_centered_at(
        pm,
        text,
        rect.x + rect.w / 2.0,
        rect.y + rect.h / 2.0,
        size,
        color,
    );
}

pub fn button(
    ui: &mut Ui,
    pm: &mut Pixmap,
    text: &str,
    rect: Rect,
    painter: &mut TextPainter,
) -> bool {
    let m = ui.m;
    let hot = ui.hovered(rect);
    panel(
        pm,
        rect,
        if hot {
            theme().button_hot
        } else {
            theme().button
        },
    );
    label_centered(pm, text, rect, m.text, theme().text, painter);
    ui.clicked_in(rect)
}

pub fn toggle(
    ui: &mut Ui,
    pm: &mut Pixmap,
    text: &str,
    rect: Rect,
    value: &mut bool,
    painter: &mut TextPainter,
) -> bool {
    let m = ui.m;
    let hot = ui.hovered(rect);
    let box_size = (rect.h - m.px(6.0)).min(m.px(18.0));
    let box_rect = Rect::new(
        rect.x,
        rect.y + (rect.h - box_size) / 2.0,
        box_size,
        box_size,
    );
    panel(
        pm,
        box_rect,
        if hot {
            theme().control_hot
        } else {
            theme().control
        },
    );
    if *value {
        panel(pm, box_rect.inset(m.px(4.0)), theme().accent);
    }
    if !text.is_empty() {
        let height = painter.measure(text, m.text).1;
        painter.draw_at(
            pm,
            text,
            box_rect.right() + m.px(8.0),
            rect.y + (rect.h - height) / 2.0,
            m.text,
            theme().text,
        );
    }
    if ui.clicked_in(rect) {
        *value = !*value;
        return true;
    }
    false
}
