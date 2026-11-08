//! Launch at sign-in, through the per-user `Run` registry key.
//!
//! The "Startup apps" page of Windows (and the Task Manager) can disable an
//! entry without deleting it, by writing a flag under `StartupApproved\Run`.
//! The state shown in the settings reads both keys, so it matches what
//! Windows will do.

use crate::ipc::wide;
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_BINARY, REG_OPTION_NON_VOLATILE, REG_SZ, RRF_RT_REG_BINARY,
    RRF_RT_REG_SZ, RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegGetValueW, RegSetValueExW,
};
use windows::core::{PCWSTR, w};

const RUN_KEY: PCWSTR = w!(r"Software\Microsoft\Windows\CurrentVersion\Run");
const APPROVED_KEY: PCWSTR = w!(r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run");
const VALUE_NAME: PCWSTR = w!("SoundDock");

/// First byte of the `StartupApproved` flag: even means enabled, odd means
/// disabled by the user in Windows.
const APPROVED_ENABLED: [u8; 12] = [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupState {
    Off,
    On,
    /// Registered, but disabled from the Windows startup apps page.
    DisabledByWindows,
}

pub fn state() -> StartupState {
    if !run_entry_exists() {
        return StartupState::Off;
    }
    let mut flag = [0u8; 12];
    let mut size = flag.len() as u32;
    let read = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            APPROVED_KEY,
            VALUE_NAME,
            RRF_RT_REG_BINARY,
            None,
            Some(flag.as_mut_ptr().cast()),
            Some(&mut size),
        )
    };
    if read == ERROR_SUCCESS && size > 0 && flag[0] % 2 == 1 {
        StartupState::DisabledByWindows
    } else {
        StartupState::On
    }
}

/// Turning it on also clears a "disabled" flag left by Windows.
pub fn set(enabled: bool) -> windows::core::Result<()> {
    if enabled {
        write_value(RUN_KEY, REG_SZ, &command_line_bytes())?;
        write_value(APPROVED_KEY, REG_BINARY, &APPROVED_ENABLED)?;
    } else {
        delete_value(RUN_KEY);
        delete_value(APPROVED_KEY);
    }
    Ok(())
}

/// Points an existing entry at the current executable, in case it was moved.
pub fn refresh_path() {
    if run_entry_exists()
        && let Err(e) = write_value(RUN_KEY, REG_SZ, &command_line_bytes())
    {
        log::warn!("updating the startup entry failed: {e}");
    }
}

fn run_entry_exists() -> bool {
    let mut size = 0u32;
    let read = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            RUN_KEY,
            VALUE_NAME,
            RRF_RT_REG_SZ,
            None,
            None,
            Some(&mut size),
        )
    };
    read == ERROR_SUCCESS
}

fn command_line_bytes() -> Vec<u8> {
    let exe = std::env::current_exe().unwrap_or_default();
    let command = format!("\"{}\"", exe.display());
    wide(&command).iter().flat_map(|unit| unit.to_le_bytes()).collect()
}

fn write_value(
    subkey: PCWSTR,
    kind: windows::Win32::System::Registry::REG_VALUE_TYPE,
    data: &[u8],
) -> windows::core::Result<()> {
    let mut key = HKEY::default();
    unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            subkey,
            None,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            None,
            &mut key,
            None,
        )
        .ok()?;
        let result = RegSetValueExW(key, VALUE_NAME, None, kind, Some(data)).ok();
        let _ = RegCloseKey(key);
        result
    }
}

fn delete_value(subkey: PCWSTR) {
    let mut key = HKEY::default();
    unsafe {
        if RegCreateKeyExW(
            HKEY_CURRENT_USER,
            subkey,
            None,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            None,
            &mut key,
            None,
        ) == ERROR_SUCCESS
        {
            let _ = RegDeleteValueW(key, VALUE_NAME);
            let _ = RegCloseKey(key);
        }
    }
}
