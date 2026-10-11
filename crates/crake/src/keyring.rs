//! What Crake keeps in the system's keyring: a tunnel's certificate
//! password and, if the user chose so, the certificate itself. Encrypted by
//! the keyring, unlocked at login; never in Crake's own files. Linux: the
//! GNOME keyring (Secret Service); Windows: Credential Manager (`sys`).

pub use crate::sys::keyring::{forget, get, set};

/// What a secret is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    CertificatePassword,
    Certificate,
    /// The gateway password (sign-in "Username and password").
    Password,
}

impl Kind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::CertificatePassword => "certificate-password",
            Self::Certificate => "certificate",
            Self::Password => "password",
        }
    }

    pub(crate) fn label(self, display: &str) -> String {
        match self {
            Self::CertificatePassword => format!("Crake: certificate password for {display}"),
            Self::Certificate => format!("Crake: certificate for {display}"),
            Self::Password => format!("Crake: password for {display}"),
        }
    }
}
