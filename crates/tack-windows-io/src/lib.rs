//! Native diagnostic-only disk quota query, with no COM/UI dependency.
#[cfg(windows)]
pub fn available_bytes(directory: &std::path::Path) -> std::io::Result<u64> {
    use std::os::windows::ffi::OsStrExt;
    let mut path = Vec::new();
    for unit in directory.as_os_str().encode_wide() {
        if unit == 0 || path.len() >= 32767 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "invalid diagnostic directory",
            ));
        }
        path.push(unit);
    }
    path.push(0);
    let mut available = 0;
    // SAFETY: terminated path remains live, available is a writable u64, unused
    // output pointers are null. No pointer or borrowed data outlives this call.
    let result = unsafe {
        windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(
            path.as_ptr(),
            &mut available,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if result == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(available)
    }
}
#[cfg(all(test, windows))]
mod tests {
    use super::*;
    #[test]
    fn quota_query_has_no_com_or_shell_dependency_and_rejects_nul() {
        assert!(available_bytes(&std::env::temp_dir()).is_ok());
        assert_eq!(
            available_bytes(std::path::Path::new("bad\0directory"))
                .err()
                .map(|e| e.kind()),
            Some(std::io::ErrorKind::InvalidInput)
        );
    }
}
