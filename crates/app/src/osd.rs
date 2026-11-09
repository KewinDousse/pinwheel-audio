//! The on-screen display window: a layered, click-through, topmost popup
//! that never takes the focus and stays out of Alt+Tab.
//!
//! It shows for the duration set, then fades out over [`FADE`]. Each new
//! content restarts the hold. The timers belong to the main window, which forwards
//! `WM_TIMER` to [`Osd::on_timer`], so all state lives on the main thread.

use crate::accent;
use pinwheel_core::i18n::Lang;
use pinwheel_core::osd::{Content, Fonts, Look, Rgb, SHADOW_MARGIN, render};
use pinwheel_core::settings::{OsdPosition, OsdScreen, OsdSettings};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tiny_skia::Pixmap;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION, CreateCompatibleDC,
    CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, GetMonitorInfoW, HBITMAP, HDC, HGDIOBJ,
    MONITOR_DEFAULTTONEAREST, MONITOR_DEFAULTTOPRIMARY, MONITORINFO, MonitorFromPoint, ReleaseDC, SelectObject,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::Shell::{
    QUNS_BUSY, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN, SHQueryUserNotificationState,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, GetCursorPos, HTTRANSPARENT, HWND_TOPMOST, KillTimer, MA_NOACTIVATE,
    RegisterClassExW, SW_HIDE, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetTimer, SetWindowPos,
    ShowWindow, ULW_ALPHA, UpdateLayeredWindow, WM_MOUSEACTIVATE, WM_NCHITTEST, WNDCLASSEXW, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};
use windows::core::w;

const FADE: Duration = Duration::from_millis(300);
const FADE_TICK_MS: u32 = 15;

pub const TIMER_HOLD: usize = 10;
pub const TIMER_FADE: usize = 11;

/// Distance from the top or bottom of the work area to the panel, in logical
/// pixels.
const EDGE_MARGIN_TOP: f32 = 48.0;
const EDGE_MARGIN_BOTTOM: f32 = 72.0;

pub struct Osd {
    hwnd: HWND,
    timer_owner: HWND,
    fonts: Fonts,
    surface: Option<Surface>,
    fade_start: Option<Instant>,
    /// Opacity of the panel before it fades.
    alpha: u8,
    /// Read once per appearance: reading it takes longer than drawing.
    accent: Option<Rgb>,
}

impl Osd {
    /// `timer_owner` is the main window, which receives the fade timers.
    pub fn new(timer_owner: HWND) -> windows::core::Result<Self> {
        let class = w!("Pinwheel.Osd");
        let instance = unsafe { GetModuleHandleW(None) }?.into();
        let wc = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            lpszClassName: class,
            ..Default::default()
        };
        unsafe { RegisterClassExW(&wc) };
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                class,
                w!("Pinwheel"),
                WS_POPUP,
                0,
                0,
                0,
                0,
                None,
                None,
                Some(instance),
                None,
            )
        }?;
        let fonts = system_fonts();
        if !fonts.has_text() {
            log::warn!("no system font found: the display will show no text");
        }
        Ok(Self {
            hwnd,
            timer_owner,
            fonts,
            surface: None,
            fade_start: None,
            alpha: 255,
            accent: None,
        })
    }

    /// Shows `content`, unless the settings say not to.
    pub fn show(&mut self, content: &Content, settings: OsdSettings, lang: Lang) {
        if !settings.enabled || (settings.hide_in_fullscreen && full_screen_app_running()) {
            return;
        }
        let monitor = match settings.screen {
            OsdScreen::Pointer => {
                let mut cursor = POINT::default();
                let _ = unsafe { GetCursorPos(&mut cursor) };
                unsafe { MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST) }
            }
            OsdScreen::Primary => unsafe { MonitorFromPoint(POINT::default(), MONITOR_DEFAULTTOPRIMARY) },
        };
        let mut info = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let _ = unsafe { GetMonitorInfoW(monitor, &mut info) };
        let (mut dpi_x, mut dpi_y) = (96, 96);
        let _ = unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) };
        let scale = dpi_x as f32 / 96.0;

        let look = Look {
            style: settings.style,
            scale,
            accent: *self.accent.get_or_insert_with(accent::on_dark),
            lang,
        };
        let pixmap = render(content, look, &self.fonts);
        let Some(surface) = Surface::new(&pixmap) else {
            log::error!("creating the display bitmap failed");
            return;
        };
        let work = info.rcWork;
        let (width, height) = (pixmap.width() as i32, pixmap.height() as i32);
        // The shadow may spill into the margin; the panel keeps its distance.
        let shadow = (SHADOW_MARGIN * scale).round() as i32;
        let x = work.left + (work.right - work.left - width) / 2;
        let y = match settings.position {
            OsdPosition::Top => work.top + (EDGE_MARGIN_TOP * scale) as i32 - shadow,
            OsdPosition::Center => work.top + (work.bottom - work.top - height) / 2,
            OsdPosition::Bottom => work.bottom - height - (EDGE_MARGIN_BOTTOM * scale) as i32 + shadow,
        };

        let alpha = (f32::from(settings.opacity) * 2.55).round() as u8;
        let screen = unsafe { GetDC(None) };
        let origin = POINT::default();
        let result = unsafe {
            UpdateLayeredWindow(
                self.hwnd,
                Some(screen),
                Some(&POINT { x, y }),
                Some(&SIZE { cx: width, cy: height }),
                Some(surface.dc),
                Some(&origin),
                COLORREF(0),
                Some(&blend(alpha)),
                ULW_ALPHA,
            )
        };
        unsafe { ReleaseDC(None, screen) };
        if let Err(e) = result {
            // What is on screen keeps its timers, so it still fades out.
            log::error!("drawing the display failed: {e}");
            return;
        }
        self.stop_timers();
        self.surface = Some(surface);
        self.alpha = alpha;
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
            // Topmost windows opened later would otherwise cover it.
            let _ = SetWindowPos(
                self.hwnd,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
            SetTimer(Some(self.timer_owner), TIMER_HOLD, settings.duration_ms, None);
        }
    }

    /// Handles the display's timers; returns `false` for other timer IDs.
    pub fn on_timer(&mut self, id: usize) -> bool {
        match id {
            TIMER_HOLD => {
                let _ = unsafe { KillTimer(Some(self.timer_owner), TIMER_HOLD) };
                self.fade_start = Some(Instant::now());
                unsafe { SetTimer(Some(self.timer_owner), TIMER_FADE, FADE_TICK_MS, None) };
            }
            TIMER_FADE => {
                let progress = self
                    .fade_start
                    .map_or(1.0, |start| start.elapsed().as_secs_f32() / FADE.as_secs_f32());
                if progress >= 1.0 {
                    self.hide();
                } else {
                    let alpha = (f32::from(self.alpha) * (1.0 - progress)) as u8;
                    // Without a source DC, only the opacity changes.
                    let _ = unsafe {
                        UpdateLayeredWindow(
                            self.hwnd,
                            None,
                            None,
                            None,
                            None,
                            None,
                            COLORREF(0),
                            Some(&blend(alpha)),
                            ULW_ALPHA,
                        )
                    };
                }
            }
            _ => return false,
        }
        true
    }

    fn hide(&mut self) {
        self.stop_timers();
        let _ = unsafe { ShowWindow(self.hwnd, SW_HIDE) };
        self.surface = None;
        self.accent = None;
    }

    fn stop_timers(&mut self) {
        let _ = unsafe { KillTimer(Some(self.timer_owner), TIMER_HOLD) };
        let _ = unsafe { KillTimer(Some(self.timer_owner), TIMER_FADE) };
        self.fade_start = None;
    }
}

/// A full-screen app, a game or a presentation, as Windows tells apps that
/// want to stay quiet.
fn full_screen_app_running() -> bool {
    unsafe { SHQueryUserNotificationState() }
        .is_ok_and(|state| [QUNS_BUSY, QUNS_RUNNING_D3D_FULL_SCREEN, QUNS_PRESENTATION_MODE].contains(&state))
}

fn system_fonts() -> Fonts {
    let dir = PathBuf::from(std::env::var_os("WINDIR").unwrap_or_else(|| "C:\\Windows".into())).join("Fonts");
    let paths = |names: &[&str]| names.iter().map(|name| dir.join(name)).collect::<Vec<_>>();
    Fonts::from_candidates(
        &paths(&["segoeui.ttf", "SegUIVar.ttf", "arial.ttf"]),
        &paths(&["seguisb.ttf", "segoeuib.ttf", "arialbd.ttf"]),
    )
}

fn blend(alpha: u8) -> BLENDFUNCTION {
    BLENDFUNCTION {
        BlendOp: AC_SRC_OVER as u8,
        BlendFlags: 0,
        SourceConstantAlpha: alpha,
        AlphaFormat: AC_SRC_ALPHA as u8,
    }
}

unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_NCHITTEST => LRESULT(HTTRANSPARENT as isize),
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

/// A memory DC holding a 32-bit premultiplied BGRA copy of the pixmap.
struct Surface {
    dc: HDC,
    bitmap: HBITMAP,
    previous: HGDIOBJ,
}

impl Surface {
    fn new(pixmap: &Pixmap) -> Option<Self> {
        let (width, height) = (pixmap.width() as i32, pixmap.height() as i32);
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                // Negative height: rows top to bottom, like the pixmap.
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        unsafe {
            let dc = CreateCompatibleDC(None);
            let mut bits = std::ptr::null_mut();
            let Ok(bitmap) = CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0) else {
                let _ = DeleteDC(dc);
                return None;
            };
            let target = std::slice::from_raw_parts_mut(bits.cast::<u8>(), (width * height * 4) as usize);
            for (dst, src) in target
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .zip(pixmap.data().as_chunks::<4>().0)
            {
                *dst = [src[2], src[1], src[0], src[3]];
            }
            let previous = SelectObject(dc, bitmap.into());
            Some(Self { dc, bitmap, previous })
        }
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.previous);
            let _ = DeleteObject(self.bitmap.into());
            let _ = DeleteDC(self.dc);
        }
    }
}
