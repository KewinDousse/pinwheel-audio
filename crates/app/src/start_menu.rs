//! The Start menu entry, so that Pinwheel can be found and started again
//! from Start without an installer.
//!
//! A shortcut in the user's Start menu folder points at the running
//! executable. The background process writes it at each start when it is
//! missing or points elsewhere, as after moving the executable.

use crate::ipc::wide;
use std::path::{Path, PathBuf};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, CoTaskMemFree, IPersistFile, STGM_READ};
use windows::Win32::UI::Shell::{FOLDERID_Programs, IShellLinkW, KF_FLAG_DEFAULT, SHGetKnownFolderPath, ShellLink};
use windows::core::{Interface, PCWSTR, Result};

pub fn refresh() {
    let Ok(exe) = std::env::current_exe() else { return };
    if let Err(e) = write_if_needed(&exe) {
        log::warn!("updating the Start menu entry failed: {e}");
    }
}

fn write_if_needed(exe: &Path) -> Result<()> {
    let path = programs_folder()?.join("Pinwheel.lnk");
    let link: IShellLinkW = unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER) }?;
    let file: IPersistFile = link.cast()?;
    let path_wide = wide(&path.display().to_string());
    if path.exists() && unsafe { file.Load(PCWSTR(path_wide.as_ptr()), STGM_READ) }.is_ok() {
        let mut target = [0u16; 1024];
        unsafe { link.GetPath(&mut target, std::ptr::null_mut(), 0) }?;
        let length = target.iter().position(|&c| c == 0).unwrap_or(target.len());
        if PathBuf::from(String::from_utf16_lossy(&target[..length])) == exe {
            return Ok(());
        }
    }
    let exe_wide = wide(&exe.display().to_string());
    let description = wide(env!("CARGO_PKG_DESCRIPTION"));
    unsafe {
        link.SetPath(PCWSTR(exe_wide.as_ptr()))?;
        link.SetDescription(PCWSTR(description.as_ptr()))?;
        file.Save(PCWSTR(path_wide.as_ptr()), true)?;
    }
    log::info!("Start menu entry written: {}", path.display());
    Ok(())
}

fn programs_folder() -> Result<PathBuf> {
    let raw = unsafe { SHGetKnownFolderPath(&FOLDERID_Programs, KF_FLAG_DEFAULT, None) }?;
    let folder = unsafe { raw.to_string() };
    unsafe { CoTaskMemFree(Some(raw.0 as *const _)) };
    Ok(PathBuf::from(folder.unwrap_or_default()))
}
