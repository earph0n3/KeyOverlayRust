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
    /// Field that currently owns the keyboard, if any.
    editing: Option<Edit>,
    /// Value typed into a field, handed to its widget on the frame the edit ends.
    committed: Option<(u64, Committed)>,
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
            editing: None,
            committed: None,
            mouse: (0.0, 0.0),
            pressed: false,
            released: false,
            active: None,
            press_origin: None,
            next_id: 0,
        }
    }
}

/// A numeric field being typed into.
struct Edit {
    id: u64,
    text: String,
    /// True until the first keystroke, so typing replaces the shown value.
    fresh: bool,
    /// Number fields take digits and a decimal point, text fields take anything.
    numeric: bool,
}

/// What a field handed back when its edit ended.
enum Committed {
    Number(f32),
    Text(String),
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

    /// True while a value box owns the keyboard.
    pub fn is_editing(&self) -> bool {
        self.editing.is_some()
    }

    fn editing_id(&self) -> Option<u64> {
        self.editing.as_ref().map(|edit| edit.id)
    }

    fn begin_edit(&mut self, id: u64, text: String, numeric: bool) {
        self.editing = Some(Edit {
            id,
            text,
            fresh: true,
            numeric,
        });
    }

    /// Routes a typed character to the focused field. Number fields take digits
    /// and a decimal point; text fields take anything printable, up to 24
    /// characters (a key label is a character or two).
    pub fn type_char(&mut self, ch: char) {
        let Some(edit) = self.editing.as_mut() else {
            return;
        };
        if edit.numeric {
            let ch = if ch == ',' { '.' } else { ch };
            if !(ch.is_ascii_digit() || ch == '.') || edit.text.len() >= 6 {
                return;
            }
        } else if ch.is_control() || edit.text.chars().count() >= 24 {
            // Control characters have their own events (Enter ends the edit).
            return;
        }
        if edit.fresh {
            edit.text.clear();
            edit.fresh = false;
        }
        edit.text.push(ch);
    }

    pub fn backspace(&mut self) {
        if let Some(edit) = self.editing.as_mut() {
            edit.fresh = false;
            edit.text.pop();
        }
    }

    /// Ends the edit; the widget that owns the field applies the value on its
    /// next draw.
    pub fn end_edit(&mut self) {
        let Some(edit) = self.editing.take() else {
            return;
        };
        let committed = if edit.numeric {
            // A half-typed number is dropped rather than applied.
            edit.text.trim().parse::<f32>().ok().map(Committed::Number)
        } else {
            Some(Committed::Text(edit.text.trim().to_string()))
        };
        if let Some(committed) = committed {
            self.committed = Some((edit.id, committed));
        }
    }

    pub fn cancel_edit(&mut self) {
        self.editing = None;
    }

    /// A standalone numeric field (not part of a slider): returns the typed
    /// value once, on the frame the edit ends.
    pub fn field(
        &mut self,
        pm: &mut Pixmap,
        rect: Rect,
        value_text: &str,
        m: Metrics,
        painter: &mut TextPainter,
    ) -> Option<f32> {
        let id = self.id();
        if self.pressed && self.editing_id() == Some(id) && !rect.contains(self.mouse) {
            self.end_edit();
        }
        let committed = match self.take_committed(id) {
            Some(Committed::Number(value)) => Some(value),
            _ => None,
        };
        // Draws the shown value, or the in-progress text and its caret.
        self.value_box(pm, id, rect, value_text, m, painter);
        if self.clicked_in(rect) && self.editing_id() != Some(id) {
            self.begin_edit(id, value_text.to_string(), true);
        }
        committed
    }

    /// True while the pointer sits over `rect`.
    pub fn hovered(&self, rect: Rect) -> bool {
        rect.contains(self.mouse)
    }

    /// True when a press and the matching release both happened inside `rect`.
    pub fn clicked_in(&self, rect: Rect) -> bool {
        self.released
            && self
                .press_origin
                .is_some_and(|origin| rect.contains(origin))
            && rect.contains(self.mouse)
    }

    /// Hands the value typed into field `id` back to its widget, once.
    fn take_committed(&mut self, id: u64) -> Option<Committed> {
        match self.committed.take() {
            Some((committed_id, value)) if committed_id == id => Some(value),
            other => {
                self.committed = other;
                None
            }
        }
    }

    /// The read-out next to a slider. It looks like a plain label until it is
    /// clicked, then it takes the keyboard and shows what is being typed.
    /// Returns true while it owns the keyboard.
    fn value_box(
        &mut self,
        pm: &mut Pixmap,
        id: u64,
        rect: Rect,
        text: &str,
        m: Metrics,
        painter: &mut TextPainter,
    ) -> bool {
        let t = theme();
        let editing = self.editing_id() == Some(id);
        let hot = rect.contains(self.mouse);
        let fill = if editing || hot {
            t.control_hot
        } else {
            t.control
        };
        panel(pm, rect, fill);
        if editing {
            fill_rect(pm, rect.x, rect.y, m.px(2.0), rect.h, t.accent);
        }
        let shown = match self.editing.as_ref().filter(|edit| edit.id == id) {
            Some(edit) => edit.text.as_str(),
            None => text,
        };
        let (width, height) = painter.measure(shown, m.text);
        warn_if_overflow(shown, width, rect.inset(m.px(6.0)));
        painter.draw_at(
            pm,
            shown,
            rect.x + (rect.w - width) / 2.0,
            rect.y + (rect.h - height) / 2.0,
            m.text,
            if editing { t.text } else { t.text_dim },
        );
        editing
    }

    /// A one-line text field, such as a key's display name. Returns the typed
    /// text once, on the frame the edit ends; `placeholder` is drawn dim while
    /// the field is empty.
    pub fn text_field(
        &mut self,
        pm: &mut Pixmap,
        rect: Rect,
        value: &str,
        placeholder: &str,
        m: Metrics,
        painter: &mut TextPainter,
    ) -> Option<String> {
        let id = self.id();
        let t = theme();
        // Clicking anywhere else commits what was typed, like the value boxes.
        if self.pressed && self.editing_id() == Some(id) && !rect.contains(self.mouse) {
            self.end_edit();
        }
        let committed = match self.take_committed(id) {
            Some(Committed::Text(text)) => Some(text),
            _ => None,
        };

        let editing = self.editing_id() == Some(id);
        let hot = rect.contains(self.mouse);
        panel(pm, rect, if hot { t.control_hot } else { t.row });
        if editing {
            fill_rect(pm, rect.x, rect.y, m.px(2.0), rect.h, t.accent);
        }
        let typed = match self.editing.as_ref().filter(|edit| edit.id == id) {
            Some(edit) => edit.text.as_str(),
            None => value,
        };
        let (shown, color) = if typed.is_empty() && !editing {
            (placeholder, t.text_dim)
        } else {
            (typed, t.text)
        };
        let area = rect.inset(m.px(8.0));
        let (width, height) = painter.measure(shown, m.small);
        warn_if_overflow(shown, width, area);
        painter.draw_at(
            pm,
            shown,
            area.x,
            rect.y + (rect.h - height) / 2.0,
            m.small,
            color,
        );

        if self.clicked_in(rect) && !editing {
            self.begin_edit(id, value.to_string(), false);
        }
        committed
    }

    /// Horizontal slider with a read-out box that can also be typed into: the
    /// bar is for coarse dragging, the box for exact values. Returns true on the
    /// frame the value changed.
    #[allow(clippy::too_many_arguments)]
    pub fn slider(
        &mut self,
        pm: &mut Pixmap,
        label: &str,
        rect: Rect,
        value: &mut f32,
        min: f32,
        max: f32,
        step: f32,
        display: impl Fn(f32) -> String,
        painter: &mut TextPainter,
    ) -> bool {
        let id = self.id();
        let m = self.m;
        let t = theme();
        let mut changed = false;

        let label_w = m.px(125.0);
        let box_w = m.px(62.0);
        let gap = m.px(8.0);
        let track = Rect::new(
            rect.x + label_w,
            rect.y + m.px(5.0),
            (rect.w - label_w - box_w - gap).max(m.px(40.0)),
            rect.h - m.px(10.0),
        );
        let value_rect = Rect::new(track.right() + gap, rect.y, box_w, rect.h);

        // Clicking anywhere else commits what was typed, like the plain fields.
        if self.pressed && self.editing_id() == Some(id) && !value_rect.contains(self.mouse) {
            self.end_edit();
        }

        let (width, height) = painter.measure(label, m.small);
        warn_if_overflow(
            label,
            width,
            Rect::new(rect.x, rect.y, label_w - m.px(8.0), rect.h),
        );
        painter.draw_at(
            pm,
            label,
            rect.x,
            rect.y + (rect.h - height) / 2.0,
            m.small,
            t.text_dim,
        );

        // Pressing the bar starts a drag that follows the pointer until the
        // button comes up.
        if self.pressed && self.editing_id() != Some(id) && track.contains(self.mouse) {
            self.active = Some(id);
        }
        if self.pressed && self.active == Some(id) {
            let fraction = ((self.mouse.0 - track.x) / track.w).clamp(0.0, 1.0);
            let next = snap(min + fraction * (max - min), min, max, step);
            if next != *value {
                *value = next;
                changed = true;
            }
        }

        // A value that was typed into the read-out box.
        if let Some(Committed::Number(typed)) = self.take_committed(id) {
            let next = snap(typed, min, max, step);
            if next != *value {
                *value = next;
                changed = true;
            }
        }

        // The bar, with the filled part showing where the value sits.
        let hot = self.active == Some(id) || track.contains(self.mouse);
        panel(pm, track, if hot { t.control_hot } else { t.control });
        let knob = m.px(8.0);
        let filled = (track.w - knob) * ((*value - min) / (max - min)).clamp(0.0, 1.0);
        if filled > 1.0 {
            fill_rect(pm, track.x, track.y, filled, track.h, t.accent);
        }

        let text = display(*value);
        let editing = self.value_box(pm, id, value_rect, &text, m, painter);

        if self.clicked_in(value_rect) && !editing {
            self.begin_edit(id, text, true);
        }

        changed
    }
}

/// Rounds a value to the nearest step and keeps it inside the range.
fn snap(value: f32, min: f32, max: f32, step: f32) -> f32 {
    let snapped = if step > 0.0 {
        (value / step).round() * step
    } else {
        value
    };
    snapped.clamp(min, max)
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Typing goes through one buffer for every field, so the two kinds have to
    /// stay apart: a key label is text, a slider read-out is a number.
    #[test]
    fn text_fields_take_letters_and_number_fields_do_not() {
        let mut ui = Ui::default();
        ui.begin(Metrics::new(1.0));

        let label = ui.id();
        ui.begin_edit(label, String::new(), false);
        for ch in "jump1".chars() {
            ui.type_char(ch);
        }
        ui.end_edit();
        assert!(
            matches!(ui.take_committed(label), Some(Committed::Text(text)) if text == "jump1"),
            "a text field must keep what was typed into it"
        );

        let number = ui.id();
        ui.begin_edit(number, "70".to_string(), true);
        for ch in "1x2z.5".chars() {
            ui.type_char(ch);
        }
        ui.end_edit();
        assert!(
            matches!(ui.take_committed(number), Some(Committed::Number(value)) if value == 12.5),
            "a number field must ignore anything but digits and a decimal point"
        );
    }
}
