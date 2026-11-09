//! Audio outputs through Core Audio, and the default output through the
//! undocumented `IPolicyConfig` (see `docs/adr/0002-ipolicyconfig.md`).
//!
//! No COM object is cached apart from the enumerator: each action asks for
//! the current default device again, so a device that disappeared, a resume
//! from sleep or a restart of the audio service cannot leave a stale object.

use pinwheel_core::devices::DeviceInfo;
use pinwheel_core::volume::{VolumeState, stepped_level};
use policy::{CLSID_POLICY_CONFIG_CLIENT, IPolicyConfig};
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{
    DEVICE_STATE, DEVICE_STATE_ACTIVE, DEVICE_STATE_DISABLED, DEVICE_STATE_UNPLUGGED, ERole, IMMDevice,
    IMMDeviceEnumerator, MMDeviceEnumerator, eCommunications, eConsole, eMultimedia, eRender,
};
use windows::Win32::System::Com::StructuredStorage::{PropVariantClear, PropVariantToStringAlloc};
use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance, CoTaskMemFree, STGM_READ};
use windows::core::{HRESULT, PCWSTR, PWSTR, Result};

pub struct Audio {
    enumerator: IMMDeviceEnumerator,
}

impl Audio {
    /// Requires COM to be initialised on the calling thread.
    pub fn new() -> Result<Self> {
        let enumerator = unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }?;
        Ok(Self { enumerator })
    }

    /// Outputs that are active, unplugged or disabled. Endpoints Windows
    /// reports as "not present" are left out: there can be dozens of them.
    pub fn outputs(&self) -> Result<Vec<DeviceInfo>> {
        let mask = DEVICE_STATE(DEVICE_STATE_ACTIVE.0 | DEVICE_STATE_UNPLUGGED.0 | DEVICE_STATE_DISABLED.0);
        let collection = unsafe { self.enumerator.EnumAudioEndpoints(eRender, mask) }?;
        let count = unsafe { collection.GetCount() }?;
        let mut outputs = Vec::with_capacity(count as usize);
        for index in 0..count {
            let device = unsafe { collection.Item(index) }?;
            match describe(&device) {
                Ok(info) => outputs.push(info),
                Err(e) => log::warn!("skipping output {index}: {e}"),
            }
        }
        Ok(outputs)
    }

    /// The output Windows uses for music and videos, which the wheel controls.
    fn default_output(&self) -> Result<IMMDevice> {
        unsafe { self.enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia) }
    }

    /// Current volume of the default output.
    pub fn volume(&self) -> Result<VolumeState> {
        let endpoint = endpoint_volume(&self.default_output()?)?;
        Ok(VolumeState {
            level: unsafe { endpoint.GetMasterVolumeLevelScalar() }?,
            muted: unsafe { endpoint.GetMute() }?.as_bool(),
        })
    }

    /// Moves the default output's volume by `notches` steps of `step_percent`.
    /// Turning the volume up also unmutes, as the Windows volume keys do.
    pub fn change_volume(&self, notches: i32, step_percent: u8) -> Result<VolumeState> {
        let endpoint = endpoint_volume(&self.default_output()?)?;
        let current = unsafe { endpoint.GetMasterVolumeLevelScalar() }?;
        let mut muted = unsafe { endpoint.GetMute() }?.as_bool();
        let level = stepped_level(current, notches, step_percent);
        unsafe { endpoint.SetMasterVolumeLevelScalar(level, std::ptr::null()) }?;
        if notches > 0 && muted {
            unsafe { endpoint.SetMute(false, std::ptr::null()) }?;
            muted = false;
        }
        Ok(VolumeState { level, muted })
    }

    /// Mutes the default output, or unmutes it. The change comes last, as
    /// a failure makes the caller try the whole action again.
    pub fn toggle_mute(&self) -> Result<VolumeState> {
        let endpoint = endpoint_volume(&self.default_output()?)?;
        let level = unsafe { endpoint.GetMasterVolumeLevelScalar() }?;
        let muted = !unsafe { endpoint.GetMute() }?.as_bool();
        unsafe { endpoint.SetMute(muted, std::ptr::null()) }?;
        Ok(VolumeState { level, muted })
    }

    /// Makes `id` the default output for apps and media, and optionally for calls.
    pub fn set_default(&self, id: &str, communications: bool) -> Result<()> {
        let policy: IPolicyConfig = unsafe { CoCreateInstance(&CLSID_POLICY_CONFIG_CLIENT, None, CLSCTX_ALL) }?;
        let wide = crate::ipc::wide(id);
        let roles: &[ERole] = if communications {
            &[eConsole, eMultimedia, eCommunications]
        } else {
            &[eConsole, eMultimedia]
        };
        for &role in roles {
            unsafe { policy.SetDefaultEndpoint(PCWSTR(wide.as_ptr()), role) }.ok()?;
        }
        Ok(())
    }

    /// ID of the default output.
    pub fn default_output_id(&self) -> Result<String> {
        device_id(&self.default_output()?)
    }
}

fn endpoint_volume(device: &IMMDevice) -> Result<IAudioEndpointVolume> {
    unsafe { device.Activate(CLSCTX_ALL, None) }
}

fn describe(device: &IMMDevice) -> Result<DeviceInfo> {
    let id = device_id(device)?;
    let active = unsafe { device.GetState() }? == DEVICE_STATE_ACTIVE;
    let name = friendly_name(device).unwrap_or_else(|_| id.clone());
    Ok(DeviceInfo { id, name, active })
}

fn device_id(device: &IMMDevice) -> Result<String> {
    let raw = unsafe { device.GetId() }?;
    take_string(raw)
}

fn friendly_name(device: &IMMDevice) -> Result<String> {
    let store = unsafe { device.OpenPropertyStore(STGM_READ) }?;
    let mut value = unsafe { store.GetValue(&PKEY_Device_FriendlyName) }?;
    let text = unsafe { PropVariantToStringAlloc(&value) }.and_then(take_string);
    let _ = unsafe { PropVariantClear(&mut value) };
    text
}

/// Copies a COM-allocated string and frees it.
fn take_string(raw: PWSTR) -> Result<String> {
    let text = unsafe { raw.to_string() };
    unsafe { CoTaskMemFree(Some(raw.0 as *const _)) };
    text.map_err(|e| windows::core::Error::new(HRESULT(0x8007_000D_u32 as i32), e.to_string()))
}

/// The interface the Windows Sound settings use to change the default
/// device. Only `SetDefaultEndpoint` is called; the methods before it are
/// declared to keep its position in the vtable and are never called, so
/// their parameters are left out.
#[allow(non_snake_case)]
mod policy {
    use windows::Win32::Media::Audio::ERole;
    use windows::core::{GUID, HRESULT, IUnknown, IUnknown_Vtbl, PCWSTR, interface};

    pub const CLSID_POLICY_CONFIG_CLIENT: GUID = GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9);

    #[interface("f8679f50-850a-41cf-9c72-430f290290c8")]
    pub unsafe trait IPolicyConfig: IUnknown {
        fn GetMixFormat(&self) -> HRESULT;
        fn GetDeviceFormat(&self) -> HRESULT;
        fn ResetDeviceFormat(&self) -> HRESULT;
        fn SetDeviceFormat(&self) -> HRESULT;
        fn GetProcessingPeriod(&self) -> HRESULT;
        fn SetProcessingPeriod(&self) -> HRESULT;
        fn GetShareMode(&self) -> HRESULT;
        fn SetShareMode(&self) -> HRESULT;
        fn GetPropertyValue(&self) -> HRESULT;
        fn SetPropertyValue(&self) -> HRESULT;
        pub fn SetDefaultEndpoint(&self, device_id: PCWSTR, role: ERole) -> HRESULT;
    }
}
