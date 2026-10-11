//! Credential Manager (generic credentials, this user, this computer). A
//! credential holds at most 2560 bytes, so a certificate is kept in parts:
//! `Crake/<tunnel>/<kind>/0`, `/1`, ...

use windows_sys::Win32::Security::Credentials::{
    CRED_MAX_CREDENTIAL_BLOB_SIZE, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC, CREDENTIALW,
    CredDeleteW, CredFree, CredReadW, CredWriteW,
};

use crate::keyring::Kind;

fn target(tunnel: &str, kind: Kind, part: usize) -> Vec<u16> {
    wide(&format!("Crake/{tunnel}/{}/{part}", kind.as_str()))
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}

pub async fn get(tunnel: &str, kind: Kind) -> Option<Vec<u8>> {
    let mut secret = Vec::new();
    for part in 0.. {
        let name = target(tunnel, kind, part);
        let mut cred: *mut CREDENTIALW = std::ptr::null_mut();
        // SAFETY: `name` is NUL-terminated; Windows allocates `cred`, which
        // is read and then freed here.
        unsafe {
            if CredReadW(name.as_ptr(), CRED_TYPE_GENERIC, 0, &raw mut cred) == 0 {
                break;
            }
            let c = &*cred;
            secret.extend_from_slice(std::slice::from_raw_parts(
                c.CredentialBlob,
                c.CredentialBlobSize as usize,
            ));
            CredFree(cred.cast());
        }
    }
    (!secret.is_empty()).then_some(secret)
}

pub async fn set(tunnel: &str, display: &str, kind: Kind, secret: &[u8]) -> Result<(), String> {
    forget(tunnel, kind).await;
    let comment = wide(&kind.label(display));
    let user = wide("crake");
    for (part, chunk) in secret
        .chunks(CRED_MAX_CREDENTIAL_BLOB_SIZE as usize)
        .enumerate()
    {
        let name = target(tunnel, kind, part);
        let cred = CREDENTIALW {
            Type: CRED_TYPE_GENERIC,
            TargetName: name.as_ptr().cast_mut(),
            Comment: comment.as_ptr().cast_mut(),
            CredentialBlobSize: chunk.len() as u32,
            CredentialBlob: chunk.as_ptr().cast_mut(),
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            UserName: user.as_ptr().cast_mut(),
            // SAFETY: a plain C struct, all zero is valid.
            ..unsafe { std::mem::zeroed() }
        };
        // SAFETY: every pointer in `cred` lives until the call returns.
        if unsafe { CredWriteW(&raw const cred, 0) } == 0 {
            let e = std::io::Error::last_os_error();
            forget(tunnel, kind).await;
            return Err(e.to_string());
        }
    }
    Ok(())
}

pub async fn forget(tunnel: &str, kind: Kind) {
    for part in 0.. {
        let name = target(tunnel, kind, part);
        // SAFETY: `name` is NUL-terminated.
        if unsafe { CredDeleteW(name.as_ptr(), CRED_TYPE_GENERIC, 0) } == 0 {
            break;
        }
    }
}
