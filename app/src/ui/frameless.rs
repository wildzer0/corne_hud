//! Windows without a system shadow.
//!
//! egui-winit asks winit for a drop shadow on every window without
//! decorations. Windows draws it around the whole window rectangle, even
//! where the window is transparent: with heads-up the hidden HUD would still
//! cast it on the windows below, and the empty mini indicator and keycast
//! all the time. winit makes the shadow by leaving one pixel of frame at the
//! top (`WM_NCCALCSIZE`); here the window is subclassed so that the client
//! area covers the whole window again, which is what winit does without the
//! shadow. On Windows 11 the system corners and border go too: the HUD has
//! its own.

/// Title of the HUD window.
pub const HUD_TITLE: &str = "Keyboard HUD";
/// Title of the mini indicator window.
pub const INDICATOR_TITLE: &str = "Keyboard HUD · Indicatore";
/// Title of the keycast window.
pub const KEYCAST_TITLE: &str = "Keyboard HUD · Keycast";

/// The transparent windows: menu, settings and the other windows keep the
/// shadow, which sets them apart from what is below.
#[cfg_attr(not(windows), allow(dead_code))]
const SHADOWLESS: [&str; 3] = [HUD_TITLE, INDICATOR_TITLE, KEYCAST_TITLE];

/// Removes the shadow from the transparent windows created so far. Cheap
/// enough for every frame: it only looks at the windows of this thread, and
/// a window already done is skipped.
#[cfg(windows)]
pub fn remove_shadows() {
    use windows_sys::Win32::System::Threading::GetCurrentThreadId;
    use windows_sys::Win32::UI::WindowsAndMessaging::EnumThreadWindows;
    // SAFETY: the callback only touches the window it is given
    unsafe { EnumThreadWindows(GetCurrentThreadId(), Some(win::visit), 0) };
}

#[cfg(not(windows))]
pub fn remove_shadows() {}

#[cfg(windows)]
mod win {
    use super::SHADOWLESS;
    use windows_sys::core::BOOL;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND,
    };
    use windows_sys::Win32::UI::Shell::{DefSubclassProc, GetWindowSubclass, SetWindowSubclass};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowTextW, IsZoomed, SetWindowPos, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOOWNERZORDER,
        SWP_NOSIZE, SWP_NOZORDER, WM_NCCALCSIZE,
    };

    /// Our subclass id: any constant works, it only has to be ours.
    const SUBCLASS_ID: usize = 0x4855_4400;

    pub unsafe extern "system" fn visit(hwnd: HWND, _: LPARAM) -> BOOL {
        let mut buf = [0u16; 64];
        let n = unsafe { GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
        let title = String::from_utf16_lossy(&buf[..n.max(0) as usize]);
        if SHADOWLESS.contains(&title.as_str()) {
            unsafe { make_shadowless(hwnd) };
        }
        1 // keep going
    }

    unsafe fn make_shadowless(hwnd: HWND) {
        let mut data = 0;
        if unsafe { GetWindowSubclass(hwnd, Some(no_frame), SUBCLASS_ID, &mut data) } != 0 {
            return;
        }
        if unsafe { SetWindowSubclass(hwnd, Some(no_frame), SUBCLASS_ID, 0) } == 0 {
            return;
        }
        // Windows 11 only; elsewhere these calls just fail
        let corners = DWMWCP_DONOTROUND;
        let border = DWMWA_COLOR_NONE;
        unsafe {
            DwmSetWindowAttribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE as u32, (&raw const corners).cast(), 4);
            DwmSetWindowAttribute(hwnd, DWMWA_BORDER_COLOR as u32, (&raw const border).cast(), 4);
            // recompute the frame now, through the subclass
            let flags = SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOOWNERZORDER | SWP_NOACTIVATE;
            SetWindowPos(hwnd, std::ptr::null_mut(), 0, 0, 0, 0, flags);
        }
    }

    /// Client area = the whole window: no frame left for the shadow.
    unsafe extern "system" fn no_frame(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        _: usize,
        _: usize,
    ) -> LRESULT {
        if msg == WM_NCCALCSIZE && wparam != 0 && unsafe { IsZoomed(hwnd) } == 0 {
            return 0;
        }
        unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
    }
}
