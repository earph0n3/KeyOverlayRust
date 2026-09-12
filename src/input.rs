//! Global input polling, Windows only.
//!
//! SFML's `Keyboard::isKeyPressed` / `Mouse::isButtonPressed` read the real time
//! state of the device, so the overlay keeps working while the game has focus.
//! On Windows that is `GetAsyncKeyState`, which is what this module uses - the
//! same call SFML makes, with the same virtual key codes.

use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_LBUTTON, VK_MBUTTON, VK_RBUTTON, VK_XBUTTON1, VK_XBUTTON2,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_SWAPBUTTON};

use crate::keys::{Input, MouseButton};

pub struct InputState {
    swap_buttons: bool,
}

impl InputState {
    pub fn new() -> Self {
        // SFML swaps left/right when the user enabled "swap primary and
        // secondary mouse buttons" in the Windows settings.
        let swap_buttons = unsafe { GetSystemMetrics(SM_SWAPBUTTON) } != 0;
        Self { swap_buttons }
    }

    pub fn is_down(&self, input: Input) -> bool {
        let vkey = match input {
            Input::Key(vkey) => vkey,
            Input::Mouse(button) => {
                (match button {
                    MouseButton::Left => {
                        if self.swap_buttons {
                            VK_RBUTTON
                        } else {
                            VK_LBUTTON
                        }
                    }
                    MouseButton::Right => {
                        if self.swap_buttons {
                            VK_LBUTTON
                        } else {
                            VK_RBUTTON
                        }
                    }
                    MouseButton::Middle => VK_MBUTTON,
                    MouseButton::XButton1 => VK_XBUTTON1,
                    MouseButton::XButton2 => VK_XBUTTON2,
                    // SFML returns false for Mouse::ButtonCount.
                    MouseButton::ButtonCount => return false,
                }) as u32
            }
        };

        // vkey 0 (`Unknown`/`KeyCount`) is never reported by GetAsyncKeyState.
        if vkey == 0 {
            return false;
        }
        vk_down(vkey)
    }
}

/// State of a raw virtual key code; also used by the settings window's
/// "press a key" capture.
pub fn vk_down(vkey: u32) -> bool {
    if vkey == 0 {
        return false;
    }
    unsafe { (GetAsyncKeyState(vkey as i32) as u16 & 0x8000) != 0 }
}
