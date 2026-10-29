//! Start at login on windows: a value under `HKEY_CURRENT_USER\...\Run`.

use std::ffi::c_void;
use std::ptr::null_mut;

use meshtown_core::autostart::{
    run_entry_state, run_value, AutostartBackend, EntryState, APPROVED_KEY, RUN_KEY, VALUE_NAME,
};
use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA, ERROR_SUCCESS};
use windows_sys::Win32::System::Registry::{
    RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ,
    RRF_RT_REG_BINARY, RRF_RT_REG_SZ,
};

/// The windows login entry backend.
pub struct WindowsAutostart;

/// The handle to pass to `autostart::install`.
pub static AUTOSTART: WindowsAutostart = WindowsAutostart;

/// A `Run` value and the `StartupApproved` value that can switch it off, under `HKEY_CURRENT_USER`.
pub(crate) struct RegistryEntry {
    pub(crate) run_key: String,
    pub(crate) approved_key: String,
    pub(crate) name: String,
}

impl RegistryEntry {
    fn system() -> Self {
        RegistryEntry {
            run_key: RUN_KEY.to_string(),
            approved_key: APPROVED_KEY.to_string(),
            name: VALUE_NAME.to_string(),
        }
    }

    pub(crate) fn state(&self) -> Result<EntryState, String> {
        let exe = current_exe()?;
        let value = read_value(&self.run_key, &self.name, RRF_RT_REG_SZ)?.map(|b| decode_sz(&b));
        let approved = read_value(&self.approved_key, &self.name, RRF_RT_REG_BINARY)?;
        Ok(run_entry_state(value.as_deref(), approved.as_deref(), &exe))
    }

    pub(crate) fn enable(&self) -> Result<(), String> {
        let value = run_value(&current_exe()?);
        write_raw(&self.run_key, &self.name, REG_SZ, &encode_sz(&value))?;
        delete_value(&self.approved_key, &self.name)
    }

    pub(crate) fn disable(&self) -> Result<(), String> {
        delete_value(&self.run_key, &self.name)
    }
}

impl AutostartBackend for WindowsAutostart {
    fn state(&self) -> Result<EntryState, String> {
        RegistryEntry::system().state()
    }

    fn enable(&self) -> Result<(), String> {
        RegistryEntry::system().enable()
    }

    fn disable(&self) -> Result<(), String> {
        RegistryEntry::system().disable()
    }
}

fn current_exe() -> Result<std::path::PathBuf, String> {
    std::env::current_exe().map_err(|e| format!("cannot find the running binary: {e}"))
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// A registry string: UTF-16 with the closing zero, as bytes.
fn encode_sz(text: &str) -> Vec<u8> {
    wide(text)
        .iter()
        .flat_map(|unit| unit.to_le_bytes())
        .collect()
}

fn decode_sz(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u16::from_le_bytes(*p))
        .collect();
    let end = units.iter().position(|&u| u == 0).unwrap_or(units.len());
    String::from_utf16_lossy(&units[..end])
}

fn fail(what: &str, code: u32) -> String {
    format!("{what} failed with registry error {code}")
}

/// Reads a value of the given type filter; a missing key or value is `None`.
fn read_value(key: &str, name: &str, flags: u32) -> Result<Option<Vec<u8>>, String> {
    let (key, name) = (wide(key), wide(name));
    let mut size = 0u32;
    // SAFETY: the key and name are zero-terminated and outlive the call; a null data
    // pointer only asks for the size, which is written to `size`.
    let code = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            name.as_ptr(),
            flags,
            null_mut(),
            null_mut(),
            &mut size,
        )
    };
    match code {
        ERROR_FILE_NOT_FOUND => Ok(None),
        ERROR_SUCCESS | ERROR_MORE_DATA => fetch(&key, &name, flags, size),
        other => Err(fail("reading the login entry", other)),
    }
}

fn fetch(key: &[u16], name: &[u16], flags: u32, size: u32) -> Result<Option<Vec<u8>>, String> {
    let mut data = vec![0u8; size as usize];
    let mut size = size;
    // SAFETY: `data` has `size` writable bytes, and `size` tells the call exactly that;
    // key and name are zero-terminated and outlive the call.
    let code = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            name.as_ptr(),
            flags,
            null_mut(),
            data.as_mut_ptr().cast::<c_void>(),
            &mut size,
        )
    };
    data.truncate(size as usize);
    match code {
        ERROR_SUCCESS => Ok(Some(data)),
        ERROR_FILE_NOT_FOUND => Ok(None),
        other => Err(fail("reading the login entry", other)),
    }
}

/// Writes a value of the given registry type, creating the key if needed.
pub(crate) fn write_raw(key: &str, name: &str, kind: u32, bytes: &[u8]) -> Result<(), String> {
    let (key, name) = (wide(key), wide(name));
    // SAFETY: the key and name are zero-terminated; `bytes` is readable for its length.
    let code = unsafe {
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            name.as_ptr(),
            kind,
            bytes.as_ptr().cast::<c_void>(),
            bytes.len() as u32,
        )
    };
    match code {
        ERROR_SUCCESS => Ok(()),
        other => Err(fail("writing the login entry", other)),
    }
}

/// Deletes a value; a missing key or value is not an error.
fn delete_value(key: &str, name: &str) -> Result<(), String> {
    let (key, name) = (wide(key), wide(name));
    // SAFETY: the key and name are zero-terminated and outlive the call.
    let code = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr()) };
    match code {
        ERROR_SUCCESS | ERROR_FILE_NOT_FOUND => Ok(()),
        other => Err(fail("removing the login entry", other)),
    }
}

#[cfg(test)]
#[path = "autostart_tests.rs"]
mod tests;
