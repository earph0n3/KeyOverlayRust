//! `config.txt` key names -> virtual key codes.
//!
//! The table is a 1:1 copy of SFML's Win32 backend
//! (`src/SFML/Window/Win32/InputImpl.cpp`, tag 2.5.0), including the member order
//! of `sf::Keyboard::Key` (SFML 2.5) and `sf::Mouse::Button`, so existing
//! `config.txt` files keep working unchanged.
//!
//! `Enum.TryParse` in SFML.Net is case sensitive and also accepts numeric
//! strings (e.g. `key1=5`), which is not supported here - use the names.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    XButton1,
    XButton2,
    ButtonCount,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input {
    /// Windows virtual-key code, polled with `GetAsyncKeyState`.
    Key(u32),
    Mouse(MouseButton),
}

pub struct ParsedKey {
    pub input: Input,
    /// Name shown when the key is not pressed, mirroring `Key.KeyLetter`:
    /// mouse buttons drop their `m` prefix (`mLeft` -> `Left`).
    pub label: String,
}

/// `sf::Keyboard::Key` -> virtual key code (SFML 2.5 `/Win32/InputImpl.cpp`).
const KEY_TABLE: &[(&str, u32)] = &[
    // SFML maps both to vkey 0, which GetAsyncKeyState never reports.
    ("Unknown", 0),
    ("A", b'A' as u32),
    ("B", b'B' as u32),
    ("C", b'C' as u32),
    ("D", b'D' as u32),
    ("E", b'E' as u32),
    ("F", b'F' as u32),
    ("G", b'G' as u32),
    ("H", b'H' as u32),
    ("I", b'I' as u32),
    ("J", b'J' as u32),
    ("K", b'K' as u32),
    ("L", b'L' as u32),
    ("M", b'M' as u32),
    ("N", b'N' as u32),
    ("O", b'O' as u32),
    ("P", b'P' as u32),
    ("Q", b'Q' as u32),
    ("R", b'R' as u32),
    ("S", b'S' as u32),
    ("T", b'T' as u32),
    ("U", b'U' as u32),
    ("V", b'V' as u32),
    ("W", b'W' as u32),
    ("X", b'X' as u32),
    ("Y", b'Y' as u32),
    ("Z", b'Z' as u32),
    ("Num0", b'0' as u32),
    ("Num1", b'1' as u32),
    ("Num2", b'2' as u32),
    ("Num3", b'3' as u32),
    ("Num4", b'4' as u32),
    ("Num5", b'5' as u32),
    ("Num6", b'6' as u32),
    ("Num7", b'7' as u32),
    ("Num8", b'8' as u32),
    ("Num9", b'9' as u32),
    ("Escape", 0x1B),
    ("LControl", 0xA2),
    ("LShift", 0xA0),
    ("LAlt", 0xA4),
    ("LSystem", 0x5B),
    ("RControl", 0xA3),
    ("RShift", 0xA1),
    ("RAlt", 0xA5),
    ("RSystem", 0x5C),
    ("Menu", 0x5D),
    ("LBracket", 0xDB),
    ("RBracket", 0xDD),
    ("Semicolon", 0xBA),
    ("Comma", 0xBC),
    ("Period", 0xBE),
    ("Quote", 0xDE),
    ("Slash", 0xBF),
    ("Backslash", 0xDC),
    ("Tilde", 0xC0),
    ("Equal", 0xBB),
    ("Hyphen", 0xBD),
    ("Space", 0x20),
    ("Enter", 0x0D),
    ("Backspace", 0x08),
    ("Tab", 0x09),
    ("PageUp", 0x21),
    ("PageDown", 0x22),
    ("End", 0x23),
    ("Home", 0x24),
    ("Insert", 0x2D),
    ("Delete", 0x2E),
    ("Add", 0x6B),
    ("Subtract", 0x6D),
    ("Multiply", 0x6A),
    ("Divide", 0x6F),
    ("Left", 0x25),
    ("Right", 0x27),
    ("Up", 0x26),
    ("Down", 0x28),
    ("Numpad0", 0x60),
    ("Numpad1", 0x61),
    ("Numpad2", 0x62),
    ("Numpad3", 0x63),
    ("Numpad4", 0x64),
    ("Numpad5", 0x65),
    ("Numpad6", 0x66),
    ("Numpad7", 0x67),
    ("Numpad8", 0x68),
    ("Numpad9", 0x69),
    ("F1", 0x70),
    ("F2", 0x71),
    ("F3", 0x72),
    ("F4", 0x73),
    ("F5", 0x74),
    ("F6", 0x75),
    ("F7", 0x76),
    ("F8", 0x77),
    ("F9", 0x78),
    ("F10", 0x79),
    ("F11", 0x7A),
    ("F12", 0x7B),
    ("F13", 0x7C),
    ("F14", 0x7D),
    ("F15", 0x7E),
    ("Pause", 0x13),
    ("KeyCount", 0),
];

const MOUSE_TABLE: &[(&str, MouseButton)] = &[
    ("Left", MouseButton::Left),
    ("Right", MouseButton::Right),
    ("Middle", MouseButton::Middle),
    ("XButton1", MouseButton::XButton1),
    ("XButton2", MouseButton::XButton2),
    ("ButtonCount", MouseButton::ButtonCount),
];

/// Mouse buttons come from the same `GetAsyncKeyState` codes SFML uses.
const MOUSE_VKEYS: &[(&str, u32)] = &[
    ("mLeft", 0x01),
    ("mRight", 0x02),
    ("mMiddle", 0x04),
    ("mXButton1", 0x05),
    ("mXButton2", 0x06),
];

/// Everything the settings window can bind, paired with its virtual key code.
pub fn capture_candidates() -> Vec<(&'static str, u32)> {
    KEY_TABLE
        .iter()
        .copied()
        .filter(|(_, vkey)| *vkey != 0)
        .chain(MOUSE_VKEYS.iter().copied())
        .collect()
}

/// Port of the `Key` constructor: try a keyboard key first, then `m` + a mouse
/// button name. Error text matches the original (`"Invalid key " + key`).
pub fn parse(name: &str) -> Result<ParsedKey, String> {
    if let Some((_, vkey)) = KEY_TABLE.iter().find(|(key, _)| *key == name) {
        return Ok(ParsedKey {
            input: Input::Key(*vkey),
            label: name.to_string(),
        });
    }
    if let Some(rest) = name.strip_prefix('m')
        && let Some((_, button)) = MOUSE_TABLE.iter().find(|(key, _)| *key == rest)
    {
        return Ok(ParsedKey {
            input: Input::Mouse(*button),
            label: rest.to_string(),
        });
    }
    Err(format!("Invalid key {name}"))
}
