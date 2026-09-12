//! Overlay rendering - port of `AppWindow.Run`'s draw block plus `Fading` and
//! `CreateItems.CreateText`.
//!
//! Everything is rasterized with tiny-skia into a single pixmap and handed to
//! the presenter, which replaces SFML's `RenderWindow` / `RenderTexture`.
//!
//! SFML draws in this order, and so does this module:
//! `Clear(backgroundColor)` -> background image -> squares + key labels ->
//! per key: key counter + bars -> fading overlay on top.

use std::path::Path;

use tiny_skia::{
    BlendMode, Color as SkColor, IntSize, Paint, Pixmap, PixmapPaint, Rect, Transform,
};

use crate::config::Color;
use crate::layout::Square;
use crate::state::KeySlot;
use crate::text::TextPainter;

pub struct Scene<'a> {
    pub squares: &'a [Square],
    /// Label per key (`displayKeyN` or the key name).
    pub labels: &'a [String],
    pub slots: &'a [KeySlot],
    pub background_color: Color,
    pub key_color: Color,
    pub bar_color: Color,
    pub outline_color: Color,
    pub font_color: Color,
    pub press_font_color: Color,
    pub counter: bool,
    /// `false` when the window is a layered window and the background may stay
    /// transparent (`transparentBackground=yes`).
    pub opaque_background: bool,
}

pub struct Renderer {
    char_size: u32,
    origin_y: f32,
    fading: Option<Pixmap>,
    background: Option<Pixmap>,
}

impl Renderer {
    /// `background_image` is the file name inside `Resources/` (`""` = none).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        char_size: u32,
        origin_y: f32,
        window_width: u32,
        ratio_y: f32,
        fading: bool,
        background_color: Color,
        background_image: &str,
        resources_dir: &Path,
    ) -> Result<Self, String> {
        // A background image is decoration, so one that moved or was deleted
        // must not stop the overlay from starting: the settings window lists
        // the name as missing instead, where it can be changed.
        let background = match background_image {
            "" => None,
            name => match load_background(&resources_dir.join(name)) {
                Ok(image) => Some(image),
                Err(message) => {
                    eprintln!("{message}");
                    None
                }
            },
        };

        Ok(Self {
            char_size,
            origin_y,
            // The original always builds the fading texture, but only draws the
            // strips into it when `fading=yes`.
            fading: if fading {
                build_fading(window_width, ratio_y, background_color)
            } else {
                None
            },
            background,
        })
    }

    pub fn draw_frame(&mut self, text: &mut TextPainter, pm: &mut Pixmap, scene: &Scene) {
        // The original clears an opaque window with the background colour. With
        // a layered window the alpha channel is meaningful, so it is kept.
        let background = scene.background_color;
        pm.fill(SkColor::from_rgba8(
            background.r,
            background.g,
            background.b,
            if scene.opaque_background {
                255
            } else {
                background.a
            },
        ));

        if let Some(image) = &self.background {
            pm.draw_pixmap(
                0,
                0,
                image.as_ref(),
                &PixmapPaint::default(),
                Transform::identity(),
                None,
            );
        }

        // Squares and key labels (`_staticDrawables`).
        for (index, (slot, square)) in scene.slots.iter().zip(scene.squares).enumerate() {
            let fill = if slot.pressed {
                scene.bar_color
            } else {
                scene.key_color
            };
            // SFML draws the outline *outside* the shape. Filling the outer
            // rectangle first would show through when the key colour is
            // transparent (`keyColor=0,0,0,0`), so build it as a frame.
            if square.outline > 0.0 {
                let outline = square.outline;
                let width = square.width();
                fill_rect(
                    pm,
                    square.x - outline,
                    square.top() - outline,
                    width,
                    outline,
                    scene.outline_color,
                );
                fill_rect(
                    pm,
                    square.x - outline,
                    square.top() + square.size,
                    width,
                    outline,
                    scene.outline_color,
                );
                fill_rect(
                    pm,
                    square.x - outline,
                    square.top(),
                    outline,
                    square.size,
                    scene.outline_color,
                );
                fill_rect(
                    pm,
                    square.x + square.size,
                    square.top(),
                    outline,
                    square.size,
                    scene.outline_color,
                );
            }
            fill_rect(pm, square.x, square.top(), square.size, square.size, fill);

            let font_color = if slot.pressed {
                scene.press_font_color
            } else {
                scene.font_color
            };
            if let Some(label) = scene.labels.get(index) {
                let position = (square.center_x(), square.center_y());
                text.draw_centered(
                    pm,
                    label,
                    position,
                    self.char_size,
                    self.origin_y,
                    font_color,
                );
            }
        }

        // Key counters and bars are drawn after the squares and labels.
        for (slot, square) in scene.slots.iter().zip(scene.squares) {
            if scene.counter {
                // Written into a stack buffer: this runs every frame.
                let mut digits = [0u8; 10];
                let counter = format_counter(&mut digits, slot.counter);
                let position = (
                    square.center_x(),
                    square.top() + square.size + self.char_size as f32,
                );
                text.draw_centered(
                    pm,
                    counter,
                    position,
                    self.char_size,
                    self.origin_y,
                    scene.font_color,
                );
            }

            for bar in &slot.bars {
                fill_rect(pm, bar.x, bar.y, bar.w, bar.h, scene.bar_color);
            }
        }

        if let Some(fading) = &self.fading {
            // The trail fades the bars into the background colour, like the
            // original. A layered window has no background colour to fade
            // into, so there the trail takes alpha away instead - the same
            // fade, expressed with per-pixel alpha - and a see-through colour
            // (alpha 1..254) trims its opacity.
            let (opacity, blend_mode) = if scene.opaque_background {
                (1.0, BlendMode::SourceOver)
            } else if scene.background_color.a == 0 {
                (1.0, BlendMode::DestinationOut)
            } else {
                (
                    scene.background_color.a as f32 / 255.0,
                    BlendMode::SourceOver,
                )
            };
            let paint = PixmapPaint {
                opacity,
                blend_mode,
                ..PixmapPaint::default()
            };
            pm.draw_pixmap(0, 0, fading.as_ref(), &paint, Transform::identity(), None);
        }
    }
}

/// Fills an axis-aligned rectangle, the way SFML's triangle-based shapes do:
/// hard edges, no anti-aliasing.
pub fn fill_rect(pm: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, color: Color) {
    let Some(rect) = Rect::from_xywh(x, y, w, h) else {
        return;
    };
    let mut paint = Paint::default();
    paint.set_color_rgba8(color.r, color.g, color.b, color.a);
    paint.anti_alias = false;
    pm.fill_rect(rect, &paint, Transform::identity(), None);
}

/// `Convert.ToString(key.Counter)` without allocating.
fn format_counter(buffer: &mut [u8; 10], mut value: u32) -> &str {
    let mut index = buffer.len();
    loop {
        index -= 1;
        buffer[index] = b'0' + (value % 10) as u8;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    std::str::from_utf8(&buffer[index..]).unwrap_or_default()
}

/// Port of `Fading.GetBackgroundColorFadingTexture`: 255 stacked strips of the
/// background colour, the first one opaque and each following one 1/255 more
/// transparent, drawn over everything else.
fn build_fading(window_width: u32, ratio_y: f32, background_color: Color) -> Option<Pixmap> {
    let strip_height = if ratio_y >= 0.5 {
        (2.0 * ratio_y) as u32
    } else {
        1
    };
    let height = (510.0 * ratio_y) as u32;
    if window_width == 0 || height == 0 || strip_height == 0 {
        return None;
    }

    let mut pm = Pixmap::new(window_width, height)?;
    for i in 0..255u32 {
        let y = strip_height * i;
        if y >= height {
            break;
        }
        let visible = strip_height.min(height - y);
        fill_rect(
            &mut pm,
            0.0,
            y as f32,
            window_width as f32,
            visible as f32,
            Color {
                a: (255 - i) as u8,
                ..background_color
            },
        );
    }
    Some(pm)
}

fn load_background(path: &Path) -> Result<Pixmap, String> {
    let image = image::open(path).map_err(|e| format!("Could not load {}: {e}", path.display()))?;
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();

    let mut data = vec![0u8; (width * height * 4) as usize];
    for (i, pixel) in rgba.pixels().enumerate() {
        let alpha = pixel[3] as u32;
        let offset = i * 4;
        data[offset] = (pixel[0] as u32 * alpha / 255) as u8;
        data[offset + 1] = (pixel[1] as u32 * alpha / 255) as u8;
        data[offset + 2] = (pixel[2] as u32 * alpha / 255) as u8;
        data[offset + 3] = alpha as u8;
    }

    let size = IntSize::from_wh(width, height)
        .ok_or_else(|| "background image is too large".to_string())?;
    Pixmap::from_vec(data, size).ok_or_else(|| "background image has an invalid size".to_string())
}
