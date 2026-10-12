use super::{DialogError, is_cancel};
use std::{
    cell::RefCell,
    ffi::{OsStr, OsString},
    os::windows::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};
use windows::{
    Win32::{
        Foundation::HWND,
        System::Com::{
            CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
            CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize,
        },
        UI::{
            Shell::{
                Common::COMDLG_FILTERSPEC, FOS_ALLOWMULTISELECT, FOS_FILEMUSTEXIST,
                FOS_FORCEFILESYSTEM, FOS_NOCHANGEDIR, FOS_OVERWRITEPROMPT, FOS_PATHMUSTEXIST,
                FOS_PICKFOLDERS, FileOpenDialog, FileSaveDialog, IFileDialog, IFileOpenDialog,
                IFileSaveDialog, IShellItem, SHCreateItemFromParsingName, SIGDN_FILESYSPATH,
            },
            WindowsAndMessaging::{GetWindowThreadProcessId, IsWindow, KillTimer, SetTimer},
        },
    },
    core::{Interface, PCWSTR, PWSTR, w},
};
const MAX_PATH_UNITS: usize = 32768;
const MAX_RESULT_BYTES: usize = 64 * 1024;
const CANCEL: i32 = 0x800704c7u32 as i32;
pub struct Request<'a> {
    pub save: bool,
    pub folder: bool,
    pub multiple: bool,
    pub filters: &'a [(&'a str, &'a str)],
    pub directory: Option<&'a Path>,
    pub name: Option<&'a OsStr>,
    /// Must belong to this process, retained alive by the calling worker.
    pub owner: Option<usize>,
    pub max_files: usize,
}
fn error(stage: &'static str, code: i32) -> DialogError {
    DialogError { stage, code }
}
fn at<T>(stage: &'static str, value: windows::core::Result<T>) -> Result<T, DialogError> {
    value.map_err(|e| error(stage, e.code().0))
}
fn wide(value: &OsStr) -> Result<Vec<u16>, DialogError> {
    let mut v = Vec::new();
    for u in value.encode_wide() {
        if u == 0 || v.len() >= MAX_PATH_UNITS - 1 {
            return Err(error("UTF-16 path", 0x80070057u32 as i32));
        }
        v.push(u);
    }
    v.push(0);
    Ok(v)
}
struct Apartment;
impl Drop for Apartment {
    fn drop(&mut self) {
        // SAFETY: constructed only after successful CoInitializeEx on this thread,
        // declared before COM objects so they release before this balanced call.
        unsafe { CoUninitialize() }
    }
}
struct TaskString(PWSTR);
impl Drop for TaskString {
    fn drop(&mut self) {
        // SAFETY: GetDisplayName returns a COM-task-allocated string, freed once.
        unsafe { CoTaskMemFree(Some(self.0.0.cast())) }
    }
}
struct CancelState {
    dialog: IFileDialog,
    cancel: *const AtomicBool,
}
thread_local! { static ACTIVE: RefCell<Option<CancelState>> = const { RefCell::new(None) }; }
struct CancelTimer(usize);
impl Drop for CancelTimer {
    fn drop(&mut self) {
        // SAFETY: timer ID came from SetTimer on the same worker thread; kill before
        // clearing the only pointer to the borrowed cancellation flag.
        unsafe {
            let _ = KillTimer(None, self.0);
        }
        ACTIVE.with(|v| {
            v.borrow_mut().take();
        });
    }
}
unsafe extern "system" fn check_cancel(_: HWND, _: u32, _: usize, _: u32) {
    // Callback runs only during Show's STA message pump. Do not retain a borrow
    // while Close might reenter the pump; COM clone is local to this apartment.
    let state = ACTIVE.with(|v| {
        v.try_borrow()
            .ok()
            .and_then(|s| s.as_ref().map(|s| (s.dialog.clone(), s.cancel)))
    });
    if let Some((dialog, cancel)) = state {
        // SAFETY: flag is borrowed by pick for the entire Show/CancelTimer scope.
        if unsafe { &*cancel }.load(Ordering::Relaxed) {
            // SAFETY: dialog is valid on this same COM apartment thread.
            unsafe {
                let _ = dialog.Close(windows::core::HRESULT(CANCEL));
            }
        }
    }
}
fn path(item: &IShellItem) -> Result<PathBuf, DialogError> {
    // SAFETY: live COM shell item; requests filesystem name, not virtual provider.
    let text = TaskString(at("selected path", unsafe {
        item.GetDisplayName(SIGDN_FILESYSPATH)
    })?);
    if text.0.is_null() {
        return Err(error("selected path", 0x80004003u32 as i32));
    }
    // SAFETY: API guarantees NUL-terminated UTF-16; scan bounded by the Windows
    // long-path limit, retaining COM allocation until conversion finishes.
    let mut length = 0;
    while length < MAX_PATH_UNITS && unsafe { *text.0.0.add(length) } != 0 {
        length += 1;
    }
    if length == MAX_PATH_UNITS {
        return Err(error("selected path too long", 0x80070057u32 as i32));
    }
    // SAFETY: length was validated against the returned terminated allocation.
    Ok(PathBuf::from(OsString::from_wide(unsafe {
        std::slice::from_raw_parts(text.0.0, length)
    })))
}
pub fn pick(r: Request<'_>, cancel: &AtomicBool) -> Result<Vec<PathBuf>, DialogError> {
    if cancel.load(Ordering::Relaxed) {
        return Ok(Vec::new());
    }
    if r.max_files == 0 || (r.save && (r.folder || r.multiple)) {
        return Err(error("request", 0x80070057u32 as i32));
    }
    let owner = HWND(r.owner.unwrap_or(0) as *mut _);
    if !owner.is_invalid() {
        let mut pid = 0;
        // SAFETY: these Win32 functions validate opaque handles without deref;
        // caller retains an Arc<Window> to prevent handle reuse during Show.
        unsafe {
            if !IsWindow(owner).as_bool() {
                return Err(error("owner window", 0x80070006u32 as i32));
            }
            GetWindowThreadProcessId(owner, Some(&mut pid));
        }
        if pid != std::process::id() {
            return Err(error("owner process", 0x80070005u32 as i32));
        }
    }
    // SAFETY: worker has no shared COM objects; initialize STA, no OLE1 DDE.
    at("initialize STA", unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE).ok()
    })?;
    let _apartment = Apartment;
    // SAFETY: built-in CLSIDs and the requested matching COM interface; instances
    // stay on this apartment and are released before Apartment drops.
    let dialog: IFileDialog = if r.save {
        let save: IFileSaveDialog = at("create save dialog", unsafe {
            CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER)
        })?;
        at("save interface", save.cast())?
    } else {
        let open: IFileOpenDialog = at("create open dialog", unsafe {
            CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)
        })?;
        at("open interface", open.cast())?
    };
    // SAFETY: all COM methods operate on a live same-thread interface. UTF-16
    // buffers below stay alive through calls; Windows copies their values.
    unsafe {
        let mut flags = at("get options", dialog.GetOptions())?
            | FOS_FORCEFILESYSTEM
            | FOS_NOCHANGEDIR
            | FOS_PATHMUSTEXIST;
        if r.save {
            flags |= FOS_OVERWRITEPROMPT;
        } else {
            flags |= FOS_FILEMUSTEXIST;
        }
        if r.folder {
            flags |= FOS_PICKFOLDERS;
        }
        if r.multiple {
            flags |= FOS_ALLOWMULTISELECT;
        }
        at("set options", dialog.SetOptions(flags))?;
        at("title", dialog.SetTitle(w!("Tack")))?;
        let filters: Vec<_> = r
            .filters
            .iter()
            .map(|(a, b)| Ok((wide(OsStr::new(a))?, wide(OsStr::new(b))?)))
            .collect::<Result<_, DialogError>>()?;
        let specs: Vec<_> = filters
            .iter()
            .map(|(a, b)| COMDLG_FILTERSPEC {
                pszName: PCWSTR(a.as_ptr()),
                pszSpec: PCWSTR(b.as_ptr()),
            })
            .collect();
        if !specs.is_empty() && !r.folder {
            at("filters", dialog.SetFileTypes(&specs))?;
        }
        if let Some(name) = r.name {
            let name = wide(name)?;
            at("suggested name", dialog.SetFileName(PCWSTR(name.as_ptr())))?;
        }
        if let Some(dir) = r.directory {
            let dir = wide(dir.as_os_str())?;
            let item: IShellItem = at(
                "initial directory",
                SHCreateItemFromParsingName(PCWSTR(dir.as_ptr()), None),
            )?;
            at("initial directory", dialog.SetFolder(&item))?;
        }
    }
    if cancel.load(Ordering::Relaxed) {
        return Ok(Vec::new());
    }
    let occupied = ACTIVE.with(|v| v.borrow().is_some());
    if occupied {
        return Err(error("nested dialog", 0x80004005u32 as i32));
    }
    ACTIVE.with(|v| {
        *v.borrow_mut() = Some(CancelState {
            dialog: dialog.clone(),
            cancel,
        })
    });
    // SAFETY: callback exists for the bounded Show scope, thread-local pointer is
    // valid, timer pump only runs on this worker. No timer exists while unused.
    let id = unsafe { SetTimer(None, 0, 100, Some(check_cancel)) };
    if id == 0 {
        ACTIVE.with(|v| {
            v.borrow_mut().take();
        });
        return Err(error("cancel timer", 0x80004005u32 as i32));
    }
    let timer = CancelTimer(id);
    // SAFETY: validated live owner, same apartment; Show supplies modal pumping.
    let shown = unsafe { dialog.Show(owner) };
    drop(timer);
    if cancel.load(Ordering::Relaxed) || shown.as_ref().is_err_and(|e| is_cancel(e.code().0)) {
        return Ok(Vec::new());
    }
    at("show", shown)?;
    let mut paths = Vec::new();
    // SAFETY: successful Show promises a result; count bounds precede iteration.
    unsafe {
        if r.multiple {
            let open: IFileOpenDialog = at("results interface", dialog.cast())?;
            let items = at("results", open.GetResults())?;
            let count = at("result count", items.GetCount())? as usize;
            if count > r.max_files {
                return Err(error("too many selected files", 0x80070057u32 as i32));
            }
            let mut bytes = 0usize;
            for i in 0..count {
                let p = path(&at("selected item", items.GetItemAt(i as u32))?)?;
                bytes = bytes.saturating_add(p.as_os_str().encode_wide().count() * 2);
                if bytes > MAX_RESULT_BYTES {
                    return Err(error("selected paths exceed 64 KiB", 0x80070057u32 as i32));
                }
                paths.push(p);
            }
        } else {
            paths.push(path(&at("result", dialog.GetResult())?)?);
        }
    }
    Ok(paths)
}
/// Diagnostic-only disk quota query, independent of PowerShell availability.
pub fn available_bytes(directory: &Path) -> Result<u64, DialogError> {
    let directory = wide(directory.as_os_str())?;
    let mut available = 0;
    // SAFETY: terminated path remains live; writable u64 output, unused outputs
    // are None. Function validates the path and returns an HRESULT on failure.
    at("free disk space", unsafe {
        windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(
            PCWSTR(directory.as_ptr()),
            Some(&mut available),
            None,
            None,
        )
    })?;
    Ok(available)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paths_preserve_unicode_and_reject_nul_before_ffi() {
        let s = OsStr::new("C:\\essai é\\東京.tack");
        let encoded = wide(s).unwrap_or_default();
        assert_eq!(encoded.last(), Some(&0));
        assert_eq!(
            OsString::from_wide(&encoded[..encoded.len().saturating_sub(1)]),
            s
        );
        assert!(wide(OsStr::new("bad\0path")).is_err());
        assert!(wide(OsStr::new(&"a".repeat(MAX_PATH_UNITS))).is_err());
    }
    #[test]
    fn cancelled_request_does_not_initialize_com_or_timer() {
        let cancelled = AtomicBool::new(true);
        let result = pick(
            Request {
                save: false,
                folder: false,
                multiple: false,
                filters: &[],
                directory: None,
                name: None,
                owner: None,
                max_files: 1,
            },
            &cancelled,
        );
        assert_eq!(result.ok(), Some(Vec::new()));
        assert!(ACTIVE.with(|v| v.borrow().is_none()));
    }
    #[test]
    fn invalid_request_and_foreign_handle_fail_before_show() {
        let stop = AtomicBool::new(false);
        let mut r = Request {
            save: true,
            folder: true,
            multiple: false,
            filters: &[],
            directory: None,
            name: None,
            owner: None,
            max_files: 1,
        };
        assert_eq!(pick(r, &stop).err().map(|e| e.stage), Some("request"));
        r = Request {
            save: false,
            folder: false,
            multiple: false,
            filters: &[],
            directory: None,
            name: None,
            owner: Some(1),
            max_files: 1,
        };
        assert_eq!(pick(r, &stop).err().map(|e| e.stage), Some("owner window"));
        assert!(ACTIVE.with(|v| v.borrow().is_none()));
    }
}
