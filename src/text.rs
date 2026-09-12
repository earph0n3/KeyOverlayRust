//! Glyph rasterization and text layout, shared by the overlay and the settings UI.
//!
//! Two metric flavours live here:
//! * `measure`/`draw_at` use the *visual* box (tight around the painted pixels),
//!   which is what layout code wants.
//! * `draw_centered` reproduces SFML's `Text::getLocalBounds().Width / 2` origin
//!   exactly, because that is what the original overlay was built around.

use std::collections::HashMap;

use fontdue::{Font, FontSettings};
use tiny_skia::{IntSize, Pixmap, PixmapPaint, Transform};

use crate::config::Color;

struct Glyph {
    /// Left edge of the bitmap.
    xmin: i32,
    /// Distance from the baseline to the top of the bitmap (FreeType `bitmap_top`).
    top: i32,
    w: usize,
    h: usize,
    advance: f32,
    cov: Vec<u8>,
}

struct Metrics {
    /// SFML's `localBounds.Width` (`maxX - minX`, with SFML's initial values).
    sfml_width: f32,
    visual_left: f32,
    visual_top: f32,
    visual_width: f32,
    visual_height: f32,
}

pub struct TextPainter {
    font: Font,
    /// Supplies glyphs the main font lacks, e.g. CJK for the Chinese UI.
    fallback: Option<Font>,
    glyphs: HashMap<(char, u32), Glyph>,
    tinted: HashMap<(char, u32, u32), Option<Pixmap>>,
    scratch: Vec<(f32, f32, char)>,
    /// Size used by the last `layout` call, needed when blitting.
    layout_size: u32,
}

impl TextPainter {
    /// `fallback` is only consulted for characters the main font has no bitmap
    /// for, so Latin text keeps the main font's metrics.
    pub fn with_fallback(font_bytes: &[u8], fallback_bytes: Option<&[u8]>) -> Result<Self, String> {
        let font = Font::from_bytes(font_bytes, FontSettings::default())
            .map_err(|error| format!("Could not load the embedded font: {error}"))?;
        let fallback = match fallback_bytes {
            Some(bytes) => Some(
                Font::from_bytes(bytes, FontSettings::default())
                    .map_err(|error| format!("Could not load the fallback font: {error}"))?,
            ),
            None => None,
        };
        Ok(Self {
            font,
            fallback,
            glyphs: HashMap::new(),
            tinted: HashMap::new(),
            scratch: Vec::new(),
            layout_size: 0,
        })
    }

    /// Visual size `(width, height)` of `text`, as painted.
    pub fn measure(&mut self, text: &str, size: u32) -> (f32, f32) {
        let metrics = self.layout(text, size);
        (metrics.visual_width, metrics.visual_height)
    }

    /// Draws `text` with the top-left of its painted pixels at `(x, y)`.
    pub fn draw_at(
        &mut self,
        pm: &mut Pixmap,
        text: &str,
        x: f32,
        y: f32,
        size: u32,
        color: Color,
    ) {
        let metrics = self.layout(text, size);
        let offset_x = x - metrics.visual_left;
        let offset_y = y - metrics.visual_top;
        self.blit(pm, offset_x, offset_y, color);
    }

    /// Draws `text` horizontally centred on `x`, vertically centred on `y`.
    pub fn draw_centered_at(
        &mut self,
        pm: &mut Pixmap,
        text: &str,
        x: f32,
        y: f32,
        size: u32,
        color: Color,
    ) {
        let metrics = self.layout(text, size);
        let offset_x = x - metrics.visual_width / 2.0 - metrics.visual_left;
        let offset_y = y - metrics.visual_height / 2.0 - metrics.visual_top;
        self.blit(pm, offset_x, offset_y, color);
    }

    /// SFML-compatible centred draw, used by the overlay:
    /// `Origin = (localBounds.Width / 2, origin_y)`.
    pub fn draw_centered(
        &mut self,
        pm: &mut Pixmap,
        text: &str,
        position: (f32, f32),
        size: u32,
        origin_y: f32,
        color: Color,
    ) {
        let metrics = self.layout(text, size);
        let offset_x = position.0 - metrics.sfml_width / 2.0;
        let offset_y = position.1 - origin_y;
        self.blit(pm, offset_x, offset_y, color);
    }

    /// Fills `scratch` with glyph placements and returns the resulting metrics.
    fn layout(&mut self, text: &str, size: u32) -> Metrics {
        let size_f = size as f32;
        let mut pen_x = 0.0f32;
        // SFML initialises its bounds to (characterSize, characterSize, 0, 0).
        let mut sfml_min_x = size_f;
        let mut sfml_max_x = 0.0f32;
        let mut visual_left = f32::MAX;
        let mut visual_top = f32::MAX;
        let mut visual_right = f32::MIN;
        let mut visual_bottom = f32::MIN;

        self.scratch.clear();
        self.layout_size = size;
        for ch in text.chars() {
            ensure_glyph(
                &self.font,
                self.fallback.as_ref(),
                &mut self.glyphs,
                ch,
                size,
            );
            let Some(glyph) = self.glyphs.get(&(ch, size)) else {
                continue;
            };
            let vertex_x = pen_x + glyph.xmin as f32;
            let vertex_y = size_f - glyph.top as f32;
            let (w, h, advance) = (glyph.w as f32, glyph.h as f32, glyph.advance);
            self.scratch.push((vertex_x, vertex_y, ch));

            sfml_min_x = sfml_min_x.min(vertex_x);
            sfml_max_x = sfml_max_x.max(vertex_x + w);
            if w > 0.0 || h > 0.0 {
                visual_left = visual_left.min(vertex_x);
                visual_top = visual_top.min(vertex_y);
                visual_right = visual_right.max(vertex_x + w);
                visual_bottom = visual_bottom.max(vertex_y + h);
            }
            pen_x += advance;
        }

        if visual_left > visual_right || visual_top > visual_bottom {
            return Metrics {
                sfml_width: (sfml_max_x - sfml_min_x).max(0.0),
                visual_left: 0.0,
                visual_top: 0.0,
                visual_width: 0.0,
                visual_height: 0.0,
            };
        }

        Metrics {
            sfml_width: (sfml_max_x - sfml_min_x).max(0.0),
            visual_left,
            visual_top,
            visual_width: visual_right - visual_left,
            visual_height: visual_bottom - visual_top,
        }
    }

    fn blit(&mut self, pm: &mut Pixmap, offset_x: f32, offset_y: f32, color: Color) {
        let color_key = color_key(color);
        let size = self.layout_size;
        for &(_, _, ch) in &self.scratch {
            ensure_tinted(&self.glyphs, &mut self.tinted, ch, size, color, color_key);
        }
        for index in 0..self.scratch.len() {
            let (vertex_x, vertex_y, ch) = self.scratch[index];
            if let Some(Some(glyph)) = self.tinted.get(&(ch, size, color_key)) {
                pm.draw_pixmap(
                    (offset_x + vertex_x).round() as i32,
                    (offset_y + vertex_y).round() as i32,
                    glyph.as_ref(),
                    &PixmapPaint::default(),
                    Transform::identity(),
                    None,
                );
            }
        }
    }
}

fn color_key(color: Color) -> u32 {
    (color.r as u32) << 24 | (color.g as u32) << 16 | (color.b as u32) << 8 | color.a as u32
}

fn ensure_glyph(
    font: &Font,
    fallback: Option<&Font>,
    glyphs: &mut HashMap<(char, u32), Glyph>,
    ch: char,
    size: u32,
) {
    if glyphs.contains_key(&(ch, size)) {
        return;
    }
    let mut glyph = rasterize(font, ch, size);
    // `Font::rasterize` hands back the `.notdef` box for characters the face
    // does not have, so ask which face actually owns the glyph instead of
    // testing whether the bitmap came out empty.
    if !ch.is_whitespace()
        && !font.has_glyph(ch)
        && let Some(fallback) = fallback
        && fallback.has_glyph(ch)
    {
        glyph = rasterize(fallback, ch, size);
    }
    glyphs.insert((ch, size), glyph);
}

fn rasterize(font: &Font, ch: char, size: u32) -> Glyph {
    let (metrics, cov) = font.rasterize(ch, size as f32);
    Glyph {
        xmin: metrics.xmin,
        top: metrics.ymin + metrics.height as i32,
        w: metrics.width,
        h: metrics.height,
        advance: metrics.advance_width,
        cov,
    }
}

fn ensure_tinted(
    glyphs: &HashMap<(char, u32), Glyph>,
    tinted: &mut HashMap<(char, u32, u32), Option<Pixmap>>,
    ch: char,
    size: u32,
    color: Color,
    key: u32,
) {
    if tinted.contains_key(&(ch, size, key)) {
        return;
    }
    let glyph = glyphs
        .get(&(ch, size))
        .and_then(|glyph| tint_glyph(glyph, color));
    tinted.insert((ch, size, key), glyph);
}

/// Premultiplied coverage -> colour, what `sf::Text` does with its fill colour.
fn tint_glyph(glyph: &Glyph, color: Color) -> Option<Pixmap> {
    if glyph.w == 0 || glyph.h == 0 {
        return None;
    }
    let mut data = vec![0u8; glyph.w * glyph.h * 4];
    for (i, &coverage) in glyph.cov.iter().enumerate() {
        if coverage == 0 {
            continue;
        }
        let alpha = coverage as u32 * color.a as u32 / 255;
        let offset = i * 4;
        data[offset] = (color.r as u32 * alpha / 255) as u8;
        data[offset + 1] = (color.g as u32 * alpha / 255) as u8;
        data[offset + 2] = (color.b as u32 * alpha / 255) as u8;
        data[offset + 3] = alpha as u8;
    }
    Pixmap::from_vec(data, IntSize::from_wh(glyph.w as u32, glyph.h as u32)?)
}
