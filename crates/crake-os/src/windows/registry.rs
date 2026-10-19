//! The registry, as Crake uses it: NRPT rules (volatile keys in HKLM) and
//! "Start at login" (the user's Run key). A key closes itself when dropped.

use std::io;

use windows_sys::Win32::Foundation::ERROR_SUCCESS;
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_ALL_ACCESS, KEY_READ, REG_DWORD, REG_MULTI_SZ,
    REG_OPTION_VOLATILE, REG_SZ, RRF_RT_REG_DWORD, RRF_RT_REG_MULTI_SZ, RRF_RT_REG_SZ, RegCloseKey,
    RegCreateKeyExW, RegDeleteKeyValueW, RegDeleteTreeW, RegEnumKeyExW, RegGetValueW,
    RegOpenKeyExW, RegSetKeyValueW, RegSetValueExW,
};

/// Room for a registry key's name (they're at most 255 characters).
const NAME_LEN: usize = 256;

/// The hive: the machine's, or the signed-in user's.
#[derive(Clone, Copy, Debug)]
pub enum Root {
    LocalMachine,
    CurrentUser,
}

impl Root {
    fn hkey(self) -> HKEY {
        match self {
            Self::LocalMachine => HKEY_LOCAL_MACHINE,
            Self::CurrentUser => HKEY_CURRENT_USER,
        }
    }
}

/// A value to write.
pub enum Value<'a> {
    Dword(u32),
    Sz(&'a str),
    MultiSz(&'a [String]),
}

impl Value<'_> {
    /// Its type and its bytes, as the registry keeps them.
    fn raw(&self) -> (u32, Vec<u8>) {
        let utf16 = |s: &str| s.encode_utf16().chain([0]).collect::<Vec<u16>>();
        let bytes = |w: Vec<u16>| w.into_iter().flat_map(u16::to_le_bytes).collect();
        match self {
            Self::Dword(n) => (REG_DWORD, n.to_le_bytes().to_vec()),
            Self::Sz(s) => (REG_SZ, bytes(utf16(s))),
            Self::MultiSz(list) => {
                let mut w: Vec<u16> = list.iter().flat_map(|s| utf16(s)).collect();
                w.push(0);
                (REG_MULTI_SZ, bytes(w))
            }
        }
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}

fn failed(what: &str, err: u32) -> io::Error {
    io::Error::other(format!("{what}: error {err}"))
}

/// An open key.
pub struct Key(HKEY);

impl Key {
    /// Make (or open) `path` as a volatile key: gone when Windows restarts.
    pub fn create_volatile(root: Root, path: &str) -> io::Result<Self> {
        let path = wide(path);
        let mut key: HKEY = std::ptr::null_mut();
        // SAFETY: a NUL-terminated path; `key` ours to write, then owned
        // by the returned Key, which closes it.
        let err = unsafe {
            RegCreateKeyExW(
                root.hkey(),
                path.as_ptr(),
                0,
                std::ptr::null(),
                REG_OPTION_VOLATILE,
                KEY_ALL_ACCESS,
                std::ptr::null(),
                &raw mut key,
                std::ptr::null_mut(),
            )
        };
        if err == ERROR_SUCCESS {
            Ok(Self(key))
        } else {
            Err(failed("creating the key", err))
        }
    }

    /// Open `path`, for reading only, or with every right.
    pub fn open(root: Root, path: &str, write: bool) -> Option<Self> {
        let path = wide(path);
        let access = if write { KEY_ALL_ACCESS } else { KEY_READ };
        let mut key: HKEY = std::ptr::null_mut();
        // SAFETY: a NUL-terminated path; `key` ours to write, then owned.
        let err = unsafe { RegOpenKeyExW(root.hkey(), path.as_ptr(), 0, access, &raw mut key) };
        (err == ERROR_SUCCESS).then_some(Self(key))
    }

    /// Write the value `name`.
    pub fn set(&self, name: &str, value: &Value) -> io::Result<()> {
        let name_w = wide(name);
        let (kind, data) = value.raw();
        // SAFETY: an open key, a NUL-terminated name, `data` as long as given.
        let err = unsafe {
            RegSetValueExW(
                self.0,
                name_w.as_ptr(),
                0,
                kind,
                data.as_ptr(),
                u32::try_from(data.len()).unwrap_or(u32::MAX),
            )
        };
        if err == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(failed(name, err))
        }
    }

    /// The names of the keys under this one.
    pub fn subkeys(&self) -> Vec<String> {
        let mut found = Vec::new();
        for i in 0.. {
            let mut name = [0u16; NAME_LEN];
            let mut len = u32::try_from(name.len()).unwrap_or(0);
            // SAFETY: `name` holds `len` characters, as the call is told.
            let err = unsafe {
                RegEnumKeyExW(
                    self.0,
                    i,
                    name.as_mut_ptr(),
                    &raw mut len,
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            };
            if err != ERROR_SUCCESS {
                break;
            }
            let len = usize::try_from(len).unwrap_or(0).min(name.len());
            found.push(String::from_utf16_lossy(&name[..len]));
        }
        found
    }
}

impl Drop for Key {
    fn drop(&mut self) {
        // SAFETY: a key this Key opened, closed once.
        unsafe { RegCloseKey(self.0) };
    }
}

/// Delete `path` and everything under it.
pub fn delete_tree(root: Root, path: &str) {
    let path = wide(path);
    // SAFETY: a NUL-terminated path.
    unsafe { RegDeleteTreeW(root.hkey(), path.as_ptr()) };
}

/// Write the value `name` of the key `path`.
pub fn set_value(root: Root, path: &str, name: &str, value: &Value) -> io::Result<()> {
    let (path_w, name_w) = (wide(path), wide(name));
    let (kind, data) = value.raw();
    // SAFETY: NUL-terminated names; `data` as long as given.
    let err = unsafe {
        RegSetKeyValueW(
            root.hkey(),
            path_w.as_ptr(),
            name_w.as_ptr(),
            kind,
            data.as_ptr().cast(),
            u32::try_from(data.len()).unwrap_or(u32::MAX),
        )
    };
    if err == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(failed(name, err))
    }
}

/// Delete the value `name` of the key `path`.
pub fn delete_value(root: Root, path: &str, name: &str) {
    let (path, name) = (wide(path), wide(name));
    // SAFETY: NUL-terminated names.
    unsafe { RegDeleteKeyValueW(root.hkey(), path.as_ptr(), name.as_ptr()) };
}

/// Whether `path` has a string value `name`.
pub fn has_string(root: Root, path: &str, name: &str) -> bool {
    let (path, name) = (wide(path), wide(name));
    let mut size = 0u32;
    // SAFETY: NUL-terminated names; only the size is asked for.
    let err = unsafe {
        RegGetValueW(
            root.hkey(),
            path.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &raw mut size,
        )
    };
    err == ERROR_SUCCESS
}

/// A DWORD value.
pub fn dword(root: Root, path: &str, name: &str) -> Option<u32> {
    let (path, name) = (wide(path), wide(name));
    let mut value: u32 = 0;
    let mut size = u32::try_from(size_of::<u32>()).unwrap_or(0);
    // SAFETY: NUL-terminated names; `value` holds the DWORD asked for,
    // `size` bytes.
    let err = unsafe {
        RegGetValueW(
            root.hkey(),
            path.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            (&raw mut value).cast(),
            &raw mut size,
        )
    };
    (err == ERROR_SUCCESS).then_some(value)
}

/// A REG_MULTI_SZ (or REG_SZ) value, as UTF-16 with its NULs.
pub fn multi_sz(root: Root, path: &str, name: &str) -> Vec<u16> {
    let (path, name) = (wide(path), wide(name));
    let flags = RRF_RT_REG_MULTI_SZ | RRF_RT_REG_SZ;
    let mut size: u32 = 0;
    // SAFETY: NUL-terminated names; a size query, `size` ours to write.
    let err = unsafe {
        RegGetValueW(
            root.hkey(),
            path.as_ptr(),
            name.as_ptr(),
            flags,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &raw mut size,
        )
    };
    if err != ERROR_SUCCESS {
        return Vec::new();
    }
    let mut data = vec![0u16; usize::try_from(size).unwrap_or(0).div_ceil(2)];
    // SAFETY: a buffer of `size` bytes, as the call is told.
    let err = unsafe {
        RegGetValueW(
            root.hkey(),
            path.as_ptr(),
            name.as_ptr(),
            flags,
            std::ptr::null_mut(),
            data.as_mut_ptr().cast(),
            &raw mut size,
        )
    };
    if err != ERROR_SUCCESS {
        return Vec::new();
    }
    data.truncate(usize::try_from(size).unwrap_or(0) / 2);
    data
}
