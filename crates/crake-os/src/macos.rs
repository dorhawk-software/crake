//! macOS: kernel lists (sysctl), each interface's byte counters,
//! CoreFoundation values the system's DNS settings hand out, the run loop
//! watching them, utun devices for tests; and (feature `app`) the app's
//! service and login item, and the displays.

use std::ffi::CStr;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

use system_configuration::core_foundation::array::CFArray;
use system_configuration::core_foundation::base::{CFType, TCFType};
use system_configuration::core_foundation::dictionary::CFDictionary;
use system_configuration::core_foundation::runloop::{
    CFRunLoop, CFRunLoopSource, kCFRunLoopCommonModes,
};
use system_configuration::core_foundation::string::CFString;

/// A kernel list by its sysctl name ("net.inet.tcp.pcblist_n"), with
/// `room` bytes more than asked for, for what's added in between.
pub fn sysctl(name: &CStr, room: usize) -> io::Result<Vec<u8>> {
    let mut len = 0;
    // SAFETY: a size query: no buffer, `len` ours to write.
    let r = unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            std::ptr::null_mut(),
            &raw mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    if r != 0 {
        return Err(io::Error::last_os_error());
    }
    len += room;
    let mut buf = vec![0u8; len];
    // SAFETY: a buffer of `len` bytes, as `len` tells the call.
    let r = unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            buf.as_mut_ptr().cast(),
            &raw mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    if r != 0 {
        return Err(io::Error::last_os_error());
    }
    buf.truncate(len);
    Ok(buf)
}

/// Each interface's bytes in and out so far, by name (its AF_LINK entry's
/// counters).
pub fn link_bytes() -> Vec<(String, u64)> {
    let mut found = Vec::new();
    let mut head: *mut libc::ifaddrs = std::ptr::null_mut();
    // SAFETY: getifaddrs fills `head` with a list we own until
    // freeifaddrs; each node, its name and an AF_LINK entry's data (its
    // if_data) stay valid until then, and are only read here.
    unsafe {
        if libc::getifaddrs(&raw mut head) != 0 {
            return found;
        }
        let mut p = head;
        while !p.is_null() {
            let ifa = &*p;
            p = ifa.ifa_next;
            if ifa.ifa_addr.is_null()
                || i32::from((*ifa.ifa_addr).sa_family) != libc::AF_LINK
                || ifa.ifa_data.is_null()
                || ifa.ifa_name.is_null()
            {
                continue;
            }
            let name = CStr::from_ptr(ifa.ifa_name).to_string_lossy().into_owned();
            let data = &*ifa.ifa_data.cast::<libc::if_data>();
            found.push((
                name,
                u64::from(data.ifi_ibytes) + u64::from(data.ifi_obytes),
            ));
        }
        libc::freeifaddrs(head);
    }
    found
}

/// A CoreFoundation array's items.
pub fn array_items(list: &CFArray) -> Vec<CFType> {
    list.iter()
        // SAFETY: an item the array holds, valid while it does; the
        // wrapper retains it for its own life.
        .map(|item| unsafe { CFType::wrap_under_get_rule(*item) })
        .collect()
}

/// A CoreFoundation dictionary's value for `key`.
pub fn dict_value(dict: &CFDictionary, key: &CFString) -> Option<CFType> {
    let item = dict.find(key.as_CFTypeRef())?;
    // SAFETY: a value the dictionary holds; the wrapper retains it.
    Some(unsafe { CFType::wrap_under_get_rule(*item) })
}

/// A CoreFoundation dictionary's keys and values.
pub fn dict_pairs(dict: &CFDictionary) -> Vec<(CFType, CFType)> {
    let (keys, values) = dict.get_keys_and_values();
    keys.into_iter()
        .zip(values)
        // SAFETY: keys and values the dictionary holds; the wrappers
        // retain them.
        .map(|(k, v)| unsafe {
            (
                CFType::wrap_under_get_rule(k),
                CFType::wrap_under_get_rule(v),
            )
        })
        .collect()
}

/// Run this thread's run loop with `source` in it, in the common modes,
/// for good.
pub fn run_loop_with(source: &CFRunLoopSource) {
    // SAFETY: CoreFoundation's constant, set when it loads, only read.
    let common = unsafe { kCFRunLoopCommonModes };
    CFRunLoop::get_current().add_source(source, common);
    CFRunLoop::run_current();
}

/// A new utun device, as openconnect makes one (the kernel control
/// API): its descriptor and its name ("utun7"). Tests use it.
pub fn open_utun() -> io::Result<(OwnedFd, String)> {
    const CONTROL: &[u8] = b"com.apple.net.utun_control";
    const NAME_ROOM: usize = 32;
    // SAFETY: socket(2) takes plain integers.
    let fd = unsafe { libc::socket(libc::PF_SYSTEM, libc::SOCK_DGRAM, libc::SYSPROTO_CONTROL) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: a fresh descriptor, open, that nothing else owns.
    let fd = unsafe { OwnedFd::from_raw_fd(fd) };
    // SAFETY: an all-zero ctl_info is a valid one (integers, a name).
    let mut info: libc::ctl_info = unsafe { std::mem::zeroed() };
    for (slot, b) in info.ctl_name.iter_mut().zip(CONTROL) {
        *slot = libc::c_char::from_ne_bytes([*b]);
    }
    // SAFETY: CTLIOCGINFO reads and writes a ctl_info, which `info` is.
    if unsafe { libc::ioctl(fd.as_raw_fd(), libc::CTLIOCGINFO, &raw mut info) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let addr = libc::sockaddr_ctl {
        sc_len: u8::try_from(size_of::<libc::sockaddr_ctl>()).unwrap_or(u8::MAX),
        sc_family: u8::try_from(libc::AF_SYSTEM).unwrap_or_default(),
        ss_sysaddr: u16::try_from(libc::AF_SYS_CONTROL).unwrap_or_default(),
        sc_id: info.ctl_id,
        sc_unit: 0,
        sc_reserved: Default::default(),
    };
    // SAFETY: connect reads a sockaddr_ctl of the length given, ours.
    let r = unsafe {
        libc::connect(
            fd.as_raw_fd(),
            (&raw const addr).cast(),
            crate::unix::socklen::<libc::sockaddr_ctl>(),
        )
    };
    if r != 0 {
        return Err(io::Error::last_os_error());
    }
    let mut name = [0u8; NAME_ROOM];
    let mut len = libc::socklen_t::try_from(name.len()).unwrap_or_default();
    // SAFETY: getsockopt writes at most `len` bytes into `name`, that long.
    let r = unsafe {
        libc::getsockopt(
            fd.as_raw_fd(),
            libc::SYSPROTO_CONTROL,
            libc::UTUN_OPT_IFNAME,
            name.as_mut_ptr().cast(),
            &raw mut len,
        )
    };
    if r != 0 {
        return Err(io::Error::last_os_error());
    }
    let name = CStr::from_bytes_until_nul(&name)
        .map_err(io::Error::other)?
        .to_string_lossy()
        .into_owned();
    Ok((fd, name))
}

/// What `fd` has to read within `wait_ms`, into `buf`: the bytes read, or
/// None when nothing came.
pub fn read_within(fd: &impl AsRawFd, buf: &mut [u8], wait_ms: libc::c_int) -> Option<usize> {
    let mut poll = libc::pollfd {
        fd: fd.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    // SAFETY: one pollfd, ours, as the count says.
    if unsafe { libc::poll(&raw mut poll, 1, wait_ms) } <= 0 {
        return None;
    }
    // SAFETY: read writes at most `buf.len()` bytes into `buf`.
    let n = unsafe { libc::read(fd.as_raw_fd(), buf.as_mut_ptr().cast(), buf.len()) };
    usize::try_from(n).ok()
}

/// The app's service and login item, registered with SMAppService so
/// System Settings lists them under Crake.
#[cfg(feature = "app")]
pub mod service {
    use std::path::{Path, PathBuf};

    use objc2::rc::Retained;
    use objc2_foundation::NSString;
    use objc2_service_management::SMAppService;
    pub use objc2_service_management::SMAppServiceStatus as Status;

    /// A launchd file in Crake.app: a daemon's (craked) or an agent's
    /// (start at login), and where the running app keeps it.
    pub struct Service(Retained<SMAppService>, PathBuf);

    impl Service {
        pub fn daemon(plist: &str) -> Self {
            // SAFETY: a constructor taking a valid string.
            let service =
                unsafe { SMAppService::daemonServiceWithPlistName(&NSString::from_str(plist)) };
            Self(service, in_app("LaunchDaemons", plist))
        }

        pub fn agent(plist: &str) -> Self {
            // SAFETY: a constructor taking a valid string.
            let service =
                unsafe { SMAppService::agentServiceWithPlistName(&NSString::from_str(plist)) };
            Self(service, in_app("LaunchAgents", plist))
        }

        /// SMAppService reads the launchd file from the running app, and
        /// crashes when the app's files are gone (a null string in
        /// `_load_plist_from_bundle`): an upgrade deletes the old Crake.app
        /// while it runs, and it crashed at every Homebrew upgrade until
        /// 2026-10-26 ("Crake quit unexpectedly").
        fn there(&self) -> Result<(), String> {
            if self.1.is_file() {
                Ok(())
            } else {
                Err(format!("{} isn't there", self.1.display()))
            }
        }

        pub fn status(&self) -> Status {
            // SAFETY: a plain call on a valid service object.
            unsafe { self.0.status() }
        }

        /// Register it: what macOS said when it refused, if it did.
        pub fn register(&self) -> Result<(), String> {
            self.there()?;
            // SAFETY: a plain call on a valid service object, whose launchd
            // file is in the app (`there`).
            unsafe { self.0.registerAndReturnError() }.map_err(|e| format!("{e:?}"))
        }

        pub fn unregister(&self) -> Result<(), String> {
            self.there()?;
            // SAFETY: a plain call on a valid service object, whose launchd
            // file is in the app (`there`).
            unsafe { self.0.unregisterAndReturnError() }.map_err(|e| format!("{e:?}"))
        }
    }

    /// `Contents/Library/<folder>/<plist>` of the app this program runs in
    /// (its program is `Contents/MacOS/<name>`).
    fn in_app(folder: &str, plist: &str) -> PathBuf {
        let exe = std::env::current_exe().unwrap_or_default();
        contents_of(&exe).join("Library").join(folder).join(plist)
    }

    /// The app's Contents folder, from its program's path.
    fn contents_of(exe: &Path) -> PathBuf {
        exe.parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .unwrap_or_default()
    }

    /// System Settings, at Login Items & Extensions.
    pub fn open_login_items() {
        // SAFETY: a class method without arguments.
        unsafe { SMAppService::openSystemSettingsLoginItems() };
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn the_launchd_file_is_looked_for_in_the_running_app() {
            let exe = std::path::Path::new("/Applications/Crake.app/Contents/MacOS/crake");
            assert_eq!(
                super::contents_of(exe).join("Library/LaunchDaemons/x.plist"),
                std::path::Path::new(
                    "/Applications/Crake.app/Contents/Library/LaunchDaemons/x.plist"
                )
            );
            // A test binary isn't in Crake.app: nothing is asked of SMAppService.
            let gone = super::Service::daemon("software.dorhawk.crake.daemon.plist");
            assert!(gone.register().is_err());
            assert!(gone.unregister().is_err());
        }
    }
}

/// The active displays' ids, at most `max`.
#[cfg(feature = "app")]
pub fn active_displays(max: usize) -> Vec<objc2_core_graphics::CGDirectDisplayID> {
    use objc2_core_graphics::{CGError, CGGetActiveDisplayList};
    let mut ids = vec![0; max];
    let mut count = 0u32;
    // SAFETY: room for `max` ids, as the call is told, and the count.
    let err = unsafe {
        CGGetActiveDisplayList(
            u32::try_from(max).unwrap_or(u32::MAX),
            ids.as_mut_ptr(),
            &raw mut count,
        )
    };
    if err != CGError::Success {
        return Vec::new();
    }
    ids.truncate(usize::try_from(count).unwrap_or_default());
    ids
}
