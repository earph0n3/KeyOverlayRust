//! Global hotkey that opens the settings window, so it is reachable while the
//! game holds the focus (the overlay itself is usually behind a fullscreen
//! game or only visible in OBS).

use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, RegisterHotKey,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{GetMessageW, MSG, WM_HOTKEY};
use winit::event_loop::EventLoopProxy;

pub const HOTKEY_LABEL: &str = "Ctrl+Alt+K";

/// Events the window loop receives from outside.
#[derive(Debug, Clone, Copy)]
pub enum UserEvent {
    ToggleSettings,
}

const HOTKEY_ID: i32 = 1;
/// `VK_K`
const HOTKEY_KEY: u32 = 0x4B;

/// Registers the hotkey on a worker thread with its own message loop; the
/// window loop is woken through `proxy`.
pub fn spawn(proxy: EventLoopProxy<UserEvent>) -> Result<(), String> {
    let (sender, receiver) = std::sync::mpsc::channel::<Result<(), String>>();
    std::thread::Builder::new()
        .name("keyoverlay-hotkey".to_string())
        .spawn(move || unsafe {
            let registered = RegisterHotKey(
                std::ptr::null_mut(),
                HOTKEY_ID,
                MOD_CONTROL | MOD_ALT | MOD_NOREPEAT,
                HOTKEY_KEY,
            );
            let _ = sender.send(if registered == 0 {
                Err(format!(
                    "Could not register {HOTKEY_LABEL} (already in use?)"
                ))
            } else {
                Ok(())
            });
            if registered == 0 {
                return;
            }

            let mut message: MSG = std::mem::zeroed();
            while GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) > 0 {
                if message.message == WM_HOTKEY {
                    let _ = proxy.send_event(UserEvent::ToggleSettings);
                }
            }
        })
        .map_err(|error| format!("Could not start the hotkey thread: {error}"))?;

    receiver
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap_or(Ok(()))
}
