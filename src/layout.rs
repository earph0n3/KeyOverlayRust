//! Key/square geometry - port of `CreateItems.CreateKeys` and
//! `CreateItems.CreateText`.

#[derive(Clone, Copy, Debug)]
pub struct Square {
    /// Left edge of the inner square (`RectangleShape.Position.X`).
    pub x: f32,
    /// Bottom edge of the inner square (`Position.Y`, origin is `(0, size)`).
    pub y: f32,
    pub size: f32,
    pub outline: f32,
}

impl Square {
    /// Top edge of the inner square (`Position.Y - Origin.Y`).
    pub fn top(&self) -> f32 {
        self.y - self.size
    }

    /// Width including the outline, which SFML draws outside the shape.
    pub fn width(&self) -> f32 {
        self.size + self.outline * 2.0
    }

    pub fn center_x(&self) -> f32 {
        self.x + self.size / 2.0
    }

    pub fn center_y(&self) -> f32 {
        self.top() + self.size / 2.0
    }
}

/// Character size in pixels: `50 * keySize / 140`.
pub fn char_size(key_size: i32) -> u32 {
    (50 * key_size / 140).max(0) as u32
}

/// `Origin.Y` of the key label: `32 * keySize / 140`.
pub fn text_origin_y(key_size: i32) -> f32 {
    32.0 * key_size as f32 / 140.0
}

/// Squares are laid out on a 480x960 design canvas; `ratio_y` is `height / 960`.
pub fn create_squares(
    key_amount: u32,
    outline: i32,
    key_size: i32,
    margin: i32,
    window_width: u32,
    ratio_y: f32,
) -> Vec<Square> {
    if key_amount == 0 {
        return Vec::new();
    }

    let outline = outline as f32;
    let size = key_size as f32;
    let margin = margin as f32;
    let window_width = window_width as f32;
    let width = size + outline * 2.0;

    // The original computes `(windowWidth - 2 * margin - width * keyAmount) /
    // (keyAmount - 1)`, which divides by zero for a single key, so that case is
    // centered instead.
    let (spacing, first_x) = if key_amount == 1 {
        (0.0, (window_width - width) / 2.0)
    } else {
        (
            (window_width - margin * 2.0 - width * key_amount as f32) / (key_amount as f32 - 1.0),
            margin + outline,
        )
    };

    (0..key_amount)
        .map(|i| Square {
            x: first_x + (width + spacing) * i as f32,
            y: 900.0 * ratio_y,
            size,
            outline,
        })
        .collect()
}
