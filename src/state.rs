//! Per-key state machine - port of `Key` and `AppWindow.MoveBars`.

use crate::layout::Square;

pub struct Bar {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

pub struct KeySlot {
    /// Frames the key has been held for; `0` while released.
    pub hold: u32,
    pub counter: u32,
    pub pressed: bool,
    pub bars: Vec<Bar>,
}

impl KeySlot {
    pub fn new() -> Self {
        Self {
            hold: 0,
            counter: 0,
            pressed: false,
            bars: Vec::new(),
        }
    }
}

/// Port of the input loop in `AppWindow.Run`: refresh the pressed state and
/// bump `hold` while a key is down.
pub fn poll(slots: &mut [KeySlot], pressed: &[bool]) {
    for (slot, &down) in slots.iter_mut().zip(pressed) {
        slot.pressed = down;
        if down {
            slot.hold += 1;
        } else {
            slot.hold = 0;
        }
    }
}

/// Port of `AppWindow.MoveBars`: a new press spawns a bar, holding stretches the
/// newest one, every bar travels up by the frame distance and fully faded bars
/// are dropped.
pub fn move_bars(slots: &mut [KeySlot], squares: &[Square], outline: i32, dt: f32, bar_speed: f32) {
    let move_dist = dt * bar_speed;
    let outline = outline as f32;

    for (slot, square) in slots.iter_mut().zip(squares) {
        if slot.hold == 1 {
            slot.bars.push(Bar {
                x: square.x - outline,
                y: square.top() - outline,
                w: square.size + outline * 2.0,
                h: move_dist,
            });
            slot.counter += 1;
        } else if slot.hold > 1
            && let Some(last) = slot.bars.last_mut()
        {
            last.h += move_dist;
        }

        for bar in slot.bars.iter_mut() {
            bar.y -= move_dist;
        }

        if let Some(first) = slot.bars.first()
            && first.y + first.h < 0.0
        {
            slot.bars.remove(0);
        }
    }
}
