//! Threads that run a Win32 message loop, as low-level hooks need.
//!
//! The thread lives until its [`MessageThread`] is dropped: the body must
//! only return once [`run_message_loop`] has seen `WM_QUIT`. Otherwise the
//! final `WM_QUIT` could reach another thread that reused the ID.

use std::sync::mpsc;
use std::thread::JoinHandle;
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, MSG, PM_NOREMOVE, PeekMessageW, PostThreadMessageW, WM_APP, WM_QUIT,
};

pub struct MessageThread {
    thread_id: u32,
    handle: Option<JoinHandle<()>>,
}

impl MessageThread {
    pub fn spawn(name: &str, body: impl FnOnce() + Send + 'static) -> std::io::Result<Self> {
        let (sender, receiver) = mpsc::channel();
        let handle = std::thread::Builder::new().name(name.into()).spawn(move || {
            let mut msg = MSG::default();
            // Creates the thread's message queue before anyone posts to it.
            let _ = unsafe { PeekMessageW(&mut msg, None, WM_APP, WM_APP, PM_NOREMOVE) };
            let _ = sender.send(unsafe { GetCurrentThreadId() });
            body();
        })?;
        let thread_id = receiver.recv().map_err(std::io::Error::other)?;
        Ok(Self {
            thread_id,
            handle: Some(handle),
        })
    }

    /// Posts `msg` to the thread's queue.
    pub fn post(&self, msg: u32) {
        let _ = unsafe { PostThreadMessageW(self.thread_id, msg, WPARAM(0), LPARAM(0)) };
    }
}

impl Drop for MessageThread {
    fn drop(&mut self) {
        self.post(WM_QUIT);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// Hands every message to `handler` until `WM_QUIT`.
pub fn run_message_loop(mut handler: impl FnMut(&MSG)) {
    let mut msg = MSG::default();
    while unsafe { GetMessageW(&mut msg, None, 0, 0) }.0 > 0 {
        handler(&msg);
        unsafe { DispatchMessageW(&msg) };
    }
}
