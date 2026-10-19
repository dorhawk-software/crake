//! Every unsafe call Crake makes (system calls, netlink, the Windows and
//! macOS APIs), each behind a safe function whose contract this crate
//! keeps itself, with its reasoning in a `// SAFETY:` comment. The rest of
//! Crake forbids `unsafe` (`#![forbid(unsafe_code)]` in each crate, and
//! scripts/check-unsafe.sh), so a review of what could break memory safety
//! is a review of this crate.

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(unix)]
pub mod unix;
