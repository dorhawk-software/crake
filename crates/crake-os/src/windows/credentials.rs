//! Credential Manager: generic credentials of this user, kept on this
//! computer, as bytes.

use std::io;

use windows_sys::Win32::Security::Credentials::{
    CRED_MAX_CREDENTIAL_BLOB_SIZE, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC, CREDENTIALW,
    CredDeleteW, CredFree, CredReadW, CredWriteW,
};

/// The most bytes one credential holds.
pub const MAX_BLOB: usize = CRED_MAX_CREDENTIAL_BLOB_SIZE as usize;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}

/// The credential `target`'s bytes, if there is one.
pub fn read(target: &str) -> Option<Vec<u8>> {
    let name = wide(target);
    let mut cred: *mut CREDENTIALW = std::ptr::null_mut();
    // SAFETY: `name` is NUL-terminated; Windows allocates `cred`, whose
    // blob is `CredentialBlobSize` bytes; it's copied, then freed here.
    unsafe {
        if CredReadW(name.as_ptr(), CRED_TYPE_GENERIC, 0, &raw mut cred) == 0 {
            return None;
        }
        let c = &*cred;
        let blob = std::slice::from_raw_parts(
            c.CredentialBlob,
            usize::try_from(c.CredentialBlobSize).unwrap_or(0),
        )
        .to_vec();
        CredFree(cred.cast());
        Some(blob)
    }
}

/// Write the credential `target` (at most MAX_BLOB bytes), replacing one
/// of that name.
pub fn write(target: &str, comment: &str, user: &str, blob: &[u8]) -> io::Result<()> {
    if blob.len() > MAX_BLOB {
        return Err(io::Error::from(io::ErrorKind::InvalidInput));
    }
    let (name, comment, user) = (wide(target), wide(comment), wide(user));
    // SAFETY: a plain C struct, all zero is valid.
    let mut cred: CREDENTIALW = unsafe { std::mem::zeroed() };
    cred.Type = CRED_TYPE_GENERIC;
    cred.TargetName = name.as_ptr().cast_mut();
    cred.Comment = comment.as_ptr().cast_mut();
    cred.CredentialBlobSize = u32::try_from(blob.len()).unwrap_or(0);
    cred.CredentialBlob = blob.as_ptr().cast_mut();
    cred.Persist = CRED_PERSIST_LOCAL_MACHINE;
    cred.UserName = user.as_ptr().cast_mut();
    // SAFETY: every pointer in `cred` lives until the call returns, and
    // CredWriteW only reads through them.
    if unsafe { CredWriteW(&raw const cred, 0) } == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

/// Delete the credential `target`: whether there was one.
pub fn delete(target: &str) -> bool {
    let name = wide(target);
    // SAFETY: `name` is NUL-terminated.
    unsafe { CredDeleteW(name.as_ptr(), CRED_TYPE_GENERIC, 0) != 0 }
}
