//! Overlay presentation.
//!
//! Two modes:
//! * `Present::Buffer` - softbuffer's DIB + `BitBlt`, i.e. an ordinary opaque
//!   window (what the C# original does).
//! * `Present::Layered` - `WS_EX_LAYERED` + `UpdateLayeredWindow`, which gives
//!   real per-pixel alpha (transparent background) and lets the mouse fall
//!   through where nothing is drawn.
//!
//! Click-through forces the layered mode: "make the window ignore the mouse"
//! relies on the layered hit-testing, and a layered window that is drawn with
//! `BitBlt` instead of `UpdateLayeredWindow` would not be visible at all.

use std::sync::Arc;

use tiny_skia::Pixmap;
use windows_sys::Win32::Foundation::{HWND, POINT, SIZE};
use windows_sys::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
    CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, HDC,
    HGDIOBJ, ReleaseDC, SelectObject,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GWL_EXSTYLE, GetWindowLongPtrW, GetWindowRect, HWND_NOTOPMOST, HWND_TOPMOST, SWP_FRAMECHANGED,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, SetWindowLongPtrW,
    SetWindowPos, ULW_ALPHA, UpdateLayeredWindow, WS_EX_LAYERED, WS_EX_TRANSPARENT,
};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

pub struct Presenter {
    hwnd: HWND,
    /// Kept alive for the lifetime of the surface.
    _context: Option<softbuffer::Context<Arc<Window>>>,
    surface: Option<softbuffer::Surface<Arc<Window>, Arc<Window>>>,
    layered: Option<Layered>,
    layered_mode: bool,
    click_through: bool,
    topmost: bool,
    size: (u32, u32),
}

impl Presenter {
    pub fn new(window: &Arc<Window>, size: (u32, u32)) -> Result<Self, String> {
        let hwnd = hwnd_of(window)?;
        let context = softbuffer::Context::new(window.clone())
            .map_err(|error| format!("Could not create the graphics context: {error}"))?;
        let mut surface = softbuffer::Surface::new(&context, window.clone())
            .map_err(|error| format!("Could not create the drawing surface: {error}"))?;
        let (width, height) = nonzero(size)?;
        surface
            .resize(width, height)
            .map_err(|error| format!("Could not resize the drawing surface: {error}"))?;

        Ok(Self {
            hwnd,
            _context: Some(context),
            surface: Some(surface),
            layered: None,
            layered_mode: false,
            click_through: false,
            topmost: false,
            size,
        })
    }

    /// Switches between the opaque blit and the layered (per-pixel alpha) path.
    /// Click-through implies the layered path.
    pub fn set_layered(&mut self, layered: bool) -> Result<(), String> {
        let layered = layered || self.click_through;
        if layered == self.layered_mode && (!layered || self.layered.is_some()) {
            // winit rewrites the extended styles when decorations change.
            // Reapply ours even when the presentation mode itself is unchanged.
            return self.apply_styles();
        }
        self.layered_mode = layered;
        if layered {
            self.layered = Some(Layered::new(self.hwnd, self.size.0, self.size.1)?);
        } else {
            self.layered = None;
        }
        self.apply_styles()
    }

    pub fn set_click_through(&mut self, on: bool) -> Result<(), String> {
        self.click_through = on;
        // A click-through window must be composited as a layered window, both
        // for the hit-testing and to keep the content visible.
        self.set_layered(on || self.layered_mode)
    }

    pub fn set_topmost(&mut self, on: bool) {
        self.topmost = on;
        let insert_after = if on { HWND_TOPMOST } else { HWND_NOTOPMOST };
        unsafe {
            SetWindowPos(
                self.hwnd,
                insert_after,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if (width, height) == self.size || width == 0 || height == 0 {
            return;
        }
        self.size = (width, height);
        if let Ok((w, h)) = nonzero(self.size)
            && let Some(surface) = self.surface.as_mut()
        {
            let _ = surface.resize(w, h);
        }
        if let Some(layered) = self.layered.as_mut() {
            let _ = layered.resize(width, height);
        }
    }

    pub fn present(&mut self, pixmap: &Pixmap) -> Result<(), String> {
        if self.size != (pixmap.width(), pixmap.height()) {
            self.resize(pixmap.width(), pixmap.height());
        }

        if self.layered_mode {
            let layered = self
                .layered
                .as_mut()
                .ok_or_else(|| "layered presentation is not initialised".to_string())?;
            return layered.present(pixmap);
        }

        let surface = self
            .surface
            .as_mut()
            .ok_or_else(|| "drawing surface is not initialised".to_string())?;
        let mut buffer = surface
            .buffer_mut()
            .map_err(|error| format!("Could not map the drawing surface: {error}"))?;
        let source = pixmap.data();
        // softbuffer wants 0x00RRGGBB.
        for (destination, pixel) in buffer.iter_mut().zip(source.as_chunks::<4>().0) {
            *destination = (pixel[0] as u32) << 16 | (pixel[1] as u32) << 8 | pixel[2] as u32;
        }
        buffer
            .present()
            .map_err(|error| format!("Could not present the frame: {error}"))
    }

    /// Writes the extended window styles for layered/click-through and asks DWM
    /// to re-evaluate the window frame.
    fn apply_styles(&self) -> Result<(), String> {
        unsafe {
            let mut style = GetWindowLongPtrW(self.hwnd, GWL_EXSTYLE) as u32;
            style &= !(WS_EX_LAYERED | WS_EX_TRANSPARENT);
            if self.layered_mode {
                style |= WS_EX_LAYERED;
            }
            if self.click_through {
                style |= WS_EX_TRANSPARENT;
            }
            SetWindowLongPtrW(self.hwnd, GWL_EXSTYLE, style as isize);
            SetWindowPos(
                self.hwnd,
                std::ptr::null_mut(),
                0,
                0,
                0,
                0,
                SWP_NOMOVE
                    | SWP_NOSIZE
                    | SWP_NOZORDER
                    | SWP_NOACTIVATE
                    | SWP_FRAMECHANGED
                    | SWP_SHOWWINDOW,
            );
        }
        Ok(())
    }
}

impl Drop for Presenter {
    fn drop(&mut self) {
        self.layered = None;
    }
}

/// 32-bit top-down DIB owned by a memory DC, uploaded with `UpdateLayeredWindow`.
struct Layered {
    hwnd: HWND,
    screen_dc: HDC,
    memory_dc: HDC,
    bitmap: isize,
    previous: HGDIOBJ,
    bits: *mut u8,
    width: u32,
    height: u32,
}

impl Layered {
    fn new(hwnd: HWND, width: u32, height: u32) -> Result<Self, String> {
        let mut layered = Self {
            hwnd,
            screen_dc: std::ptr::null_mut(),
            memory_dc: std::ptr::null_mut(),
            bitmap: 0,
            previous: std::ptr::null_mut(),
            bits: std::ptr::null_mut(),
            width: 0,
            height: 0,
        };
        layered.resize(width, height)?;
        Ok(layered)
    }

    fn resize(&mut self, width: u32, height: u32) -> Result<(), String> {
        if width == 0 || height == 0 {
            return Err("cannot create a zero-sized layered surface".to_string());
        }
        self.release();
        unsafe {
            self.screen_dc = GetDC(std::ptr::null_mut());
            if self.screen_dc.is_null() {
                return Err("GetDC failed".to_string());
            }
            self.memory_dc = CreateCompatibleDC(self.screen_dc);
            if self.memory_dc.is_null() {
                self.release();
                return Err("CreateCompatibleDC failed".to_string());
            }

            let header = BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width as i32,
                // Negative height = top-down rows, matching the pixmap layout.
                biHeight: -(height as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            };
            let info = BITMAPINFO {
                bmiHeader: header,
                bmiColors: [Default::default()],
            };
            let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
            let bitmap = CreateDIBSection(
                self.memory_dc,
                &info,
                DIB_RGB_COLORS,
                &mut bits,
                std::ptr::null_mut(),
                0,
            );
            if bitmap.is_null() || bits.is_null() {
                self.release();
                return Err("CreateDIBSection failed".to_string());
            }
            self.bitmap = bitmap as isize;
            self.bits = bits as *mut u8;
            self.previous = SelectObject(self.memory_dc, bitmap);
            self.width = width;
            self.height = height;
        }
        Ok(())
    }

    fn present(&mut self, pixmap: &Pixmap) -> Result<(), String> {
        if (pixmap.width(), pixmap.height()) != (self.width, self.height) {
            self.resize(pixmap.width(), pixmap.height())?;
        }

        let destination = unsafe {
            std::slice::from_raw_parts_mut(self.bits, (self.width * self.height * 4) as usize)
        };
        // tiny-skia stores premultiplied RGBA, the DIB wants premultiplied BGRA.
        for (out, pixel) in destination
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(pixmap.data().as_chunks::<4>().0)
        {
            out[0] = pixel[2];
            out[1] = pixel[1];
            out[2] = pixel[0];
            out[3] = pixel[3];
        }

        let mut rect = Default::default();
        unsafe {
            GetWindowRect(self.hwnd, &mut rect);
        }
        let position = POINT {
            x: rect.left,
            y: rect.top,
        };
        let source = POINT { x: 0, y: 0 };
        let size = SIZE {
            cx: self.width as i32,
            cy: self.height as i32,
        };
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };

        let updated = unsafe {
            UpdateLayeredWindow(
                self.hwnd,
                self.screen_dc,
                &position,
                &size,
                self.memory_dc,
                &source,
                0,
                &blend,
                ULW_ALPHA,
            )
        };
        if updated == 0 {
            return Err("UpdateLayeredWindow failed".to_string());
        }
        Ok(())
    }

    fn release(&mut self) {
        unsafe {
            if !self.memory_dc.is_null() {
                if self.bitmap != 0 {
                    SelectObject(self.memory_dc, self.previous);
                    DeleteObject(self.bitmap as HGDIOBJ);
                }
                DeleteDC(self.memory_dc);
            }
            if !self.screen_dc.is_null() {
                ReleaseDC(std::ptr::null_mut(), self.screen_dc);
            }
        }
        self.memory_dc = std::ptr::null_mut();
        self.screen_dc = std::ptr::null_mut();
        self.bitmap = 0;
        self.bits = std::ptr::null_mut();
        self.width = 0;
        self.height = 0;
    }
}

impl Drop for Layered {
    fn drop(&mut self) {
        self.release();
    }
}

fn hwnd_of(window: &Arc<Window>) -> Result<HWND, String> {
    let handle = window
        .window_handle()
        .map_err(|error| format!("Could not get the window handle: {error}"))?;
    match handle.as_raw() {
        RawWindowHandle::Win32(handle) => Ok(handle.hwnd.get() as HWND),
        _ => Err("unexpected window handle type".to_string()),
    }
}

fn nonzero(size: (u32, u32)) -> Result<(std::num::NonZeroU32, std::num::NonZeroU32), String> {
    let width =
        std::num::NonZeroU32::new(size.0).ok_or_else(|| "window has no width".to_string())?;
    let height =
        std::num::NonZeroU32::new(size.1).ok_or_else(|| "window has no height".to_string())?;
    Ok((width, height))
}
