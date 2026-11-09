//! The update check: a thread asks GitHub for the latest release, then
//! posts [`WM_UPDATE_CHECKED`] to the main window, which takes the result
//! with [`take_result`].

use crate::ipc::{WM_UPDATE_CHECKED, wide};
use pinwheel_core::update::{self, Release};
use std::ffi::c_void;
use std::sync::{Mutex, PoisonError};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::Networking::WinHttp::{
    INTERNET_DEFAULT_HTTPS_PORT, WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, WINHTTP_FLAG_SECURE, WINHTTP_QUERY_FLAG_NUMBER,
    WINHTTP_QUERY_STATUS_CODE, WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpOpenRequest,
    WinHttpQueryHeaders, WinHttpReadData, WinHttpReceiveResponse, WinHttpSendRequest, WinHttpSetTimeouts,
};
use windows::Win32::UI::WindowsAndMessaging::PostMessageW;
use windows::core::{PCWSTR, w};

/// A newer release, or `None` when this one is the latest.
pub type CheckResult = Result<Option<Release>, Failure>;

pub enum Failure {
    /// No network yet, a timeout: worth trying again soon.
    NoAnswer(String),
    /// An answer that is not the release: trying again soon would get the same.
    BadAnswer(String),
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::NoAnswer(e) => write!(f, "no answer: {e}"),
            Self::BadAnswer(e) => f.write_str(e),
        }
    }
}

impl From<windows::core::Error> for Failure {
    fn from(e: windows::core::Error) -> Self {
        Self::NoAnswer(e.to_string())
    }
}

static RESULT: Mutex<Option<CheckResult>> = Mutex::new(None);

/// An answer larger than this is not GitHub's.
const MAX_ANSWER: usize = 1 << 20;
const TIMEOUT_MS: i32 = 15_000;

pub fn start(main_window: HWND) {
    let main = main_window.0 as isize;
    let spawned = std::thread::Builder::new().name("update-check".into()).spawn(move || {
        let result = latest_release_answer()
            .and_then(|json| update::newer_release(&json, env!("CARGO_PKG_VERSION")).map_err(Failure::BadAnswer));
        *RESULT.lock().unwrap_or_else(PoisonError::into_inner) = Some(result);
        let _ = unsafe { PostMessageW(Some(HWND(main as *mut _)), WM_UPDATE_CHECKED, WPARAM(0), LPARAM(0)) };
    });
    if let Err(e) = spawned {
        log::warn!("starting the update check failed: {e}");
    }
}

pub fn take_result() -> Option<CheckResult> {
    RESULT.lock().unwrap_or_else(PoisonError::into_inner).take()
}

/// A WinHTTP handle, closed on drop.
struct Handle(*mut c_void);

impl Handle {
    fn new(raw: *mut c_void) -> windows::core::Result<Self> {
        if raw.is_null() {
            Err(windows::core::Error::from_thread())
        } else {
            Ok(Self(raw))
        }
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        let _ = unsafe { WinHttpCloseHandle(self.0) };
    }
}

/// WinHTTP rather than an HTTP crate: it comes with Windows, and follows
/// the system proxy and certificates.
fn latest_release_answer() -> Result<String, Failure> {
    let agent = wide(&format!("Pinwheel/{}", env!("CARGO_PKG_VERSION")));
    let session = Handle::new(unsafe {
        WinHttpOpen(
            PCWSTR(agent.as_ptr()),
            WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
            PCWSTR::null(),
            PCWSTR::null(),
            0,
        )
    })?;
    unsafe { WinHttpSetTimeouts(session.0, TIMEOUT_MS, TIMEOUT_MS, TIMEOUT_MS, TIMEOUT_MS) }?;
    let connection =
        Handle::new(unsafe { WinHttpConnect(session.0, w!("api.github.com"), INTERNET_DEFAULT_HTTPS_PORT, 0) })?;
    let path = wide(&update::latest_release_path());
    let request = Handle::new(unsafe {
        WinHttpOpenRequest(
            connection.0,
            w!("GET"),
            PCWSTR(path.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            std::ptr::null(),
            WINHTTP_FLAG_SECURE,
        )
    })?;
    let headers: Vec<u16> = "Accept: application/vnd.github+json".encode_utf16().collect();
    unsafe { WinHttpSendRequest(request.0, Some(&headers), None, 0, 0, 0) }?;
    unsafe { WinHttpReceiveResponse(request.0, std::ptr::null_mut()) }?;

    let mut status = 0u32;
    let mut size = size_of::<u32>() as u32;
    unsafe {
        WinHttpQueryHeaders(
            request.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            PCWSTR::null(),
            Some((&raw mut status).cast()),
            &mut size,
            std::ptr::null_mut(),
        )
    }?;
    if status != 200 {
        return Err(Failure::BadAnswer(format!("GitHub answered {status}")));
    }

    let mut body = Vec::new();
    let mut buffer = [0u8; 8192];
    loop {
        let mut read = 0u32;
        unsafe { WinHttpReadData(request.0, buffer.as_mut_ptr().cast(), buffer.len() as u32, &mut read) }?;
        if read == 0 {
            break;
        }
        body.extend_from_slice(&buffer[..read as usize]);
        if body.len() > MAX_ANSWER {
            return Err(Failure::BadAnswer("the answer is too large".into()));
        }
    }
    String::from_utf8(body).map_err(|e| Failure::BadAnswer(e.to_string()))
}
