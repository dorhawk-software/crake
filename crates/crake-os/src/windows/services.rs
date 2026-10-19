//! The service manager: a running service's process.

use windows_sys::Win32::System::Services::{
    CloseServiceHandle, OpenSCManagerW, OpenServiceW, QueryServiceStatusEx, SC_MANAGER_CONNECT,
    SC_STATUS_PROCESS_INFO, SERVICE_QUERY_STATUS, SERVICE_STATUS_PROCESS,
};

/// The process of the running service `name`.
pub fn pid(name: &str) -> Option<u32> {
    let name: Vec<u16> = name.encode_utf16().chain([0]).collect();
    // SAFETY: handles are closed before returning; `status` is sized as
    // the call is told; `name` is NUL-terminated.
    unsafe {
        let manager = OpenSCManagerW(std::ptr::null(), std::ptr::null(), SC_MANAGER_CONNECT);
        if manager.is_null() {
            return None;
        }
        let service = OpenServiceW(manager, name.as_ptr(), SERVICE_QUERY_STATUS);
        let mut status: SERVICE_STATUS_PROCESS = std::mem::zeroed();
        let mut needed = 0;
        let ok = !service.is_null()
            && QueryServiceStatusEx(
                service,
                SC_STATUS_PROCESS_INFO,
                (&raw mut status).cast(),
                u32::try_from(size_of::<SERVICE_STATUS_PROCESS>()).unwrap_or(0),
                &raw mut needed,
            ) != 0;
        if !service.is_null() {
            CloseServiceHandle(service);
        }
        CloseServiceHandle(manager);
        (ok && status.dwProcessId != 0).then_some(status.dwProcessId)
    }
}
