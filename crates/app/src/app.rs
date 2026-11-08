//! The background process: a hidden top-level window that receives the
//! tray, shortcut, hook, power and session messages.
//!
//! A hidden top-level window rather than a message-only one: message-only
//! windows miss broadcasts such as `TaskbarCreated`.
//!
//! All state lives in [`App`] on the main thread. The window procedure
//! borrows it with `try_borrow_mut`, since some Win32 calls dispatch
//! messages re-entrantly (the tray menu runs its own modal loop).

use crate::audio::Audio;
use crate::hook::HookThread;
use crate::hotkeys::Hotkeys;
use crate::ipc::{self, InstanceLock, WM_OPEN_SETTINGS, WM_PREVIEW_OSD, WM_RELOAD, WM_TRAY, WM_VOLUME_NOTCHES};
use crate::osd::Osd;
use crate::tray::{self, MenuCommand, Tray};
use crate::{paths, startup};
use sounddock_core::devices::{Resolution, resolve};
use sounddock_core::osd::Content;
use sounddock_core::settings::{self, LoadIssue, Settings};
use sounddock_core::status;
use std::cell::RefCell;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::RemoteDesktop::{NOTIFY_FOR_THIS_SESSION, WTSRegisterSessionNotification};
use windows::Win32::UI::HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext};
use windows::Win32::UI::WindowsAndMessaging::{
    ASFW_ANY, AllowSetForegroundWindow, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
    KillTimer, MSG, PBT_APMRESUMEAUTOMATIC, PostQuitMessage, RegisterClassExW, SetTimer, TranslateMessage,
    WINDOW_EX_STYLE, WM_DESTROY, WM_HOTKEY, WM_LBUTTONUP, WM_POWERBROADCAST, WM_RBUTTONUP, WM_TIMER,
    WM_WTSSESSION_CHANGE, WNDCLASSEXW, WS_OVERLAPPED, WTS_SESSION_UNLOCK,
};
use windows::core::w;

const TIMER_HOTKEY_RETRY: usize = 20;
const TIMER_TRAY_RETRY: usize = 21;
const HOTKEY_RETRY_MS: u32 = 30_000;
const TRAY_RETRY_MS: u32 = 2_000;

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
    static TASKBAR_CREATED: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

struct App {
    hwnd: HWND,
    settings: Settings,
    /// Created on first use and dropped after a failure, so a slow audio
    /// service at sign-in or a restart of it is recovered from.
    audio: Option<Audio>,
    hotkeys: Hotkeys,
    osd: Option<Osd>,
    tray: Tray,
    /// `None` when the thread could not start: shortcuts still work.
    hook: Option<HookThread>,
}

/// Runs the background process until "Quit".
pub fn run() {
    let Some(_lock) = InstanceLock::acquire(ipc::MAIN_MUTEX) else {
        // Launching it again opens the settings of the running instance.
        ipc::notify_main(WM_OPEN_SETTINGS);
        return;
    };
    unsafe {
        // The manifest already asks for it; this covers a missing manifest.
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }

    let hwnd = match create_main_window() {
        Ok(hwnd) => hwnd,
        Err(e) => {
            log::error!("creating the main window failed: {e}");
            return;
        }
    };
    TASKBAR_CREATED.set(Tray::taskbar_created_message());
    let _ = unsafe { WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION) };

    let settings_path = paths::settings_file();
    let first_run = !settings_path.exists();
    let (settings, issue) = settings::load(&settings_path);

    let osd = Osd::new(hwnd)
        .inspect_err(|e| log::error!("creating the display failed: {e}"))
        .ok();
    let mut app = App {
        hwnd,
        settings,
        audio: None,
        hotkeys: Hotkeys::default(),
        osd,
        tray: Tray::new(hwnd),
        hook: HookThread::start(hwnd)
            .inspect_err(|e| log::error!("starting the wheel hook failed: {e}"))
            .ok(),
    };
    app.report_load_issue(issue);
    app.register_hotkeys();
    if !app.tray.show() {
        log::warn!("the notification area is not ready, retrying");
        unsafe { SetTimer(Some(hwnd), TIMER_TRAY_RETRY, TRAY_RETRY_MS, None) };
    }
    startup::refresh_path();
    APP.set(Some(app));

    if first_run {
        open_settings();
    }

    let mut msg = MSG::default();
    while unsafe { GetMessageW(&mut msg, None, 0, 0) }.0 > 0 {
        unsafe {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    // Drops the tray icon and stops the hook thread.
    APP.set(None);
    log::info!("SoundDock stopped");
}

fn create_main_window() -> windows::core::Result<HWND> {
    let instance = unsafe { GetModuleHandleW(None) }?.into();
    let wc = WNDCLASSEXW {
        cbSize: size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(window_proc),
        hInstance: instance,
        lpszClassName: ipc::MAIN_CLASS,
        ..Default::default()
    };
    unsafe { RegisterClassExW(&wc) };
    // Never shown.
    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            ipc::MAIN_CLASS,
            w!("SoundDock"),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(instance),
            None,
        )
    }
}

fn open_settings() {
    let Ok(exe) = std::env::current_exe() else { return };
    // Lets the new process bring its window to the front.
    let _ = unsafe { AllowSetForegroundWindow(ASFW_ANY) };
    if let Err(e) = std::process::Command::new(exe).arg(ipc::SETTINGS_ARG).spawn() {
        log::error!("opening the settings failed: {e}");
    }
}

impl App {
    fn with_audio<T>(&mut self, action: impl Fn(&Audio) -> windows::core::Result<T>) -> Option<T> {
        // One retry with a fresh enumerator covers a restarted audio service.
        for attempt in 0..2 {
            if self.audio.is_none() {
                self.audio = Audio::new().inspect_err(|e| log::warn!("audio unavailable: {e}")).ok();
            }
            let audio = self.audio.as_ref()?;
            match action(audio) {
                Ok(value) => return Some(value),
                Err(e) => {
                    log::warn!("audio call failed (attempt {}): {e}", attempt + 1);
                    self.audio = None;
                }
            }
        }
        None
    }

    fn show(&mut self, content: &Content) {
        let style = self.settings.osd;
        if let Some(osd) = self.osd.as_mut() {
            osd.show(content, style);
        }
    }

    fn on_volume_notches(&mut self, notches: i32) {
        let step = self.settings.volume_step;
        if let Some(volume) = self.with_audio(|audio| audio.change_volume(notches, step)) {
            self.show(&Content::Volume(volume));
        }
    }

    fn on_hotkey(&mut self, index: usize) {
        let Some(binding) = self.settings.outputs.get(index).cloned() else {
            return;
        };
        let Some(devices) = self.with_audio(Audio::outputs) else {
            return;
        };
        let target = match resolve(&binding.id, &binding.name, &devices) {
            Resolution::Ready(device) => device.clone(),
            Resolution::Moved(device) => {
                log::info!(
                    "\"{}\" has a new ID ({} -> {}), updating the settings",
                    binding.name,
                    binding.id,
                    device.id
                );
                let device = device.clone();
                self.rebind(index, &device.id, &device.name);
                device
            }
            Resolution::Unavailable | Resolution::Unknown => {
                self.show(&Content::Notice(format!("{} : déconnectée", binding.name)));
                return;
            }
        };

        let switched = self.with_audio(|audio| audio.set_default(&target.id, binding.communications));
        // A silent failure of the undocumented API would show up here.
        let now = self.with_audio(Audio::default_output_id);
        if switched.is_some() && now.as_deref() == Some(target.id.as_str()) {
            log::info!("default output: {}", target.name);
            self.show(&Content::Output(target.name));
        } else {
            log::error!("switching to \"{}\" failed (default is now {now:?})", target.name);
            self.show(&Content::Notice(format!("Impossible de passer sur {}", target.name)));
        }
    }

    /// Saves the new ID of a moved output. This is the only write the
    /// background process makes to the settings, and it follows a key press.
    fn rebind(&mut self, index: usize, id: &str, name: &str) {
        let output = &mut self.settings.outputs[index];
        output.id = id.to_owned();
        output.name = name.to_owned();
        if let Err(e) = settings::save(&paths::settings_file(), &self.settings) {
            log::error!("saving the settings failed: {e}");
        }
        self.register_hotkeys();
    }

    fn reload(&mut self) {
        let (settings, issue) = settings::load(&paths::settings_file());
        self.settings = settings;
        self.report_load_issue(issue);
        self.register_hotkeys();
    }

    fn report_load_issue(&mut self, issue: Option<LoadIssue>) {
        match issue {
            None => {}
            Some(LoadIssue::Corrupt { backup, error }) => {
                log::error!("unreadable settings ({error}), kept as {}", backup.display());
                self.show(&Content::Notice(
                    "Paramètres illisibles : une copie a été gardée".into(),
                ));
            }
            Some(LoadIssue::Io(e)) => log::error!("reading the settings failed: {e}"),
        }
    }

    fn register_hotkeys(&mut self) {
        let status = self.hotkeys.register_all(self.hwnd, &self.settings.outputs);
        self.write_status(&status);
        if self.hotkeys.has_refused() {
            unsafe { SetTimer(Some(self.hwnd), TIMER_HOTKEY_RETRY, HOTKEY_RETRY_MS, None) };
        }
    }

    fn write_status(&self, status: &status::Status) {
        if let Err(e) = status::save(&paths::status_file(), status) {
            log::warn!("writing the status failed: {e}");
        }
    }

    fn on_timer(&mut self, id: usize) {
        if self.osd.as_mut().is_some_and(|osd| osd.on_timer(id)) {
            return;
        }
        match id {
            TIMER_HOTKEY_RETRY => {
                if let Some(status) = self.hotkeys.retry_refused(self.hwnd, &self.settings.outputs) {
                    self.write_status(&status);
                }
                if !self.hotkeys.has_refused() {
                    let _ = unsafe { KillTimer(Some(self.hwnd), TIMER_HOTKEY_RETRY) };
                }
            }
            TIMER_TRAY_RETRY if self.tray.show() => {
                let _ = unsafe { KillTimer(Some(self.hwnd), TIMER_TRAY_RETRY) };
            }
            _ => {}
        }
    }

    fn reinstall_hook(&mut self) {
        if let Some(hook) = &self.hook {
            hook.reinstall();
        }
    }

    fn preview(&mut self) {
        self.reload();
        if let Some(volume) = self.with_audio(Audio::volume) {
            self.show(&Content::Volume(volume));
        }
    }
}

/// Runs `action` on the app unless it is already borrowed further up the
/// stack (re-entrant dispatch), in which case the message is dropped.
fn with_app(action: impl FnOnce(&mut App)) {
    APP.with(|cell| match cell.try_borrow_mut() {
        Ok(mut guard) => {
            if let Some(app) = guard.as_mut() {
                action(app);
            }
        }
        Err(_) => log::warn!("message dropped: app busy"),
    });
}

unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_VOLUME_NOTCHES => with_app(|app| app.on_volume_notches(wparam.0 as isize as i32)),
        WM_HOTKEY => {
            if let Some(index) = Hotkeys::output_index(wparam.0) {
                with_app(|app| app.on_hotkey(index));
            }
        }
        WM_TRAY => match lparam.0 as u32 {
            WM_LBUTTONUP => open_settings(),
            WM_RBUTTONUP => match tray::show_menu(hwnd) {
                Some(MenuCommand::Settings) => open_settings(),
                Some(MenuCommand::Quit) => {
                    let _ = unsafe { DestroyWindow(hwnd) };
                }
                None => {}
            },
            _ => {}
        },
        WM_OPEN_SETTINGS => open_settings(),
        WM_RELOAD => with_app(App::reload),
        WM_PREVIEW_OSD => with_app(App::preview),
        WM_TIMER => with_app(|app| app.on_timer(wparam.0)),
        WM_POWERBROADCAST if wparam.0 as u32 == PBT_APMRESUMEAUTOMATIC => with_app(App::reinstall_hook),
        WM_WTSSESSION_CHANGE if wparam.0 as u32 == WTS_SESSION_UNLOCK => with_app(App::reinstall_hook),
        WM_DESTROY => unsafe { PostQuitMessage(0) },
        _ if msg != 0 && msg == TASKBAR_CREATED.get() => with_app(|app| {
            app.tray.hide();
            app.tray.show();
        }),
        _ => return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
    LRESULT(0)
}
