//! Windows-only observation and activation of a verified project's Editor window.
#![allow(unsafe_code)]

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::sync_channel,
};
use std::thread;

use tokio::sync::watch;
use windows_sys::Win32::Foundation::{
    FILETIME, HANDLE, HWND, INVALID_HANDLE_VALUE, WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows_sys::Win32::Storage::FileSystem::{
    FILE_NOTIFY_CHANGE_DIR_NAME, FILE_NOTIFY_CHANGE_FILE_NAME, FindCloseChangeNotification,
    FindFirstChangeNotificationW, FindNextChangeNotification, SYNCHRONIZE,
};
use windows_sys::Win32::System::Threading::{
    CreateEventW, GetProcessTimes, INFINITE, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    QueryFullProcessImageNameW, SetEvent, WaitForSingleObject,
};
use windows_sys::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, EVENT_OBJECT_CREATE, EVENT_OBJECT_DESTROY, EVENT_OBJECT_HIDE,
    EVENT_OBJECT_NAMECHANGE, EnumWindows, GA_ROOT, GetAncestor, GetClassNameW, GetForegroundWindow,
    GetMenu, GetWindowThreadProcessId, IsIconic, IsWindowVisible, MSG, MWMO_INPUTAVAILABLE,
    MsgWaitForMultipleObjectsEx, OBJID_WINDOW, PM_NOREMOVE, PM_REMOVE, PeekMessageW, QS_ALLINPUT,
    SW_RESTORE, SetForegroundWindow, ShowWindowAsync, WINEVENT_OUTOFCONTEXT,
    WINEVENT_SKIPOWNPROCESS, WM_QUIT,
};

use crate::{ProcessEvidence, file_identity_key, inspect_unity_installation, observe_processes};

const DIRECTORY_TARGET_LIMIT: usize = 20;
const PROCESS_WAIT_LIMIT: usize = 22;
const WINDOW_LIMIT: usize = 256;

thread_local! {
    static DIRTY: Cell<bool> = const { Cell::new(false) };
    static UNITY_WINDOWS: RefCell<BTreeSet<usize>> = const { RefCell::new(BTreeSet::new()) };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnityWindowState {
    Closed,
    Opening,
    Open,
    Ready,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnityWindowFocus {
    Activated,
    WindowNotFound,
    Rejected,
}

/// One desktop message loop and a last-value snapshot. Idle observation makes no queries.
pub struct UnityWindowMonitor {
    control: Arc<Control>,
    reader: UnityWindowReader,
}

struct Control {
    event: OwnedHandle,
    stopping: AtomicBool,
}

impl Control {
    fn wake(&self) {
        // SAFETY: This is our owned event; the worker also holds an Arc.
        unsafe { SetEvent(self.event.as_raw_handle()) };
    }
}

#[derive(Clone)]
pub struct UnityWindowReader(watch::Receiver<UnityWindowSnapshot>);

#[derive(Clone, Debug)]
pub struct UnityWindowSnapshot {
    pub version: u32,
    pub states: Vec<UnityWindowState>,
}

impl UnityWindowReader {
    pub fn snapshot(&self) -> UnityWindowSnapshot {
        self.0.borrow().clone()
    }

    pub async fn changed(&mut self, after: u32) -> Option<UnityWindowSnapshot> {
        loop {
            {
                let value = self.0.borrow_and_update();
                if value.version != after {
                    return Some(value.clone());
                }
            }
            self.0.changed().await.ok()?;
        }
    }
}

impl UnityWindowMonitor {
    pub fn start(roots: Vec<PathBuf>) -> io::Result<Self> {
        // SAFETY: An unnamed, auto-reset event without inherited access.
        let event = unsafe { CreateEventW(null(), 0, 0, null()) };
        if event.is_null() {
            return Err(io::Error::last_os_error());
        }
        let control = Arc::new(Control {
            // SAFETY: CreateEventW returned one owned handle.
            event: unsafe { OwnedHandle::from_raw_handle(event) },
            stopping: AtomicBool::new(false),
        });
        let (sender, receiver) = watch::channel(UnityWindowSnapshot {
            version: 0,
            states: vec![UnityWindowState::Unavailable; roots.len()],
        });
        let (ready, started) = sync_channel(1);
        let worker = control.clone();
        thread::Builder::new()
            .name("alcomd-unity-windows".into())
            .spawn(move || monitor(roots, sender, ready, &worker))?;
        started.recv().map_err(io::Error::other)??;
        Ok(Self {
            control,
            reader: UnityWindowReader(receiver),
        })
    }

    pub fn reader(&self) -> UnityWindowReader {
        self.reader.clone()
    }

    pub fn refresh(&self) {
        self.control.wake();
    }
}

impl Drop for UnityWindowMonitor {
    fn drop(&mut self) {
        self.control.stopping.store(true, Ordering::Release);
        self.control.wake();
    }
}

fn monitor(
    roots: Vec<PathBuf>,
    sender: watch::Sender<UnityWindowSnapshot>,
    ready: std::sync::mpsc::SyncSender<io::Result<()>>,
    control: &Control,
) {
    let mut message = MSG::default();
    // SAFETY: This worker owns the message queue and hook. The callback runs on
    // this thread and only records invalidation in its thread-local storage.
    unsafe { PeekMessageW(&mut message, null_mut(), 0, 0, PM_NOREMOVE) };
    let Some(_lifecycle_hook) = WindowHook::register(EVENT_OBJECT_CREATE, EVENT_OBJECT_HIDE) else {
        let _ = ready.send(Err(io::Error::last_os_error()));
        return;
    };
    // A splash/container can acquire its final title after its first show.
    let _title_hook = WindowHook::register(EVENT_OBJECT_NAMECHANGE, EVENT_OBJECT_NAMECHANGE);
    let identities: Vec<_> = roots
        .iter()
        .map(|root| file_identity_key(root).ok())
        .collect();
    let mut directories: Vec<_> = roots
        .iter()
        .take(DIRECTORY_TARGET_LIMIT)
        .map(|root| DirectoryWatch::new(root))
        .collect();
    let mut processes = refresh(&identities, &sender);
    if ready.send(Ok(())).is_err() {
        return;
    }
    loop {
        let mut handles = vec![control.event.as_raw_handle()];
        handles.extend(processes.iter().map(AsRawHandle::as_raw_handle));
        let mut directory_handles = Vec::new();
        for (index, directory) in directories.iter().enumerate() {
            for (is_root, handle) in [(true, &directory.root), (false, &directory.temp)] {
                if let Some(handle) = handle {
                    handles.push(handle.0);
                    directory_handles.push((index, is_root));
                }
            }
        }
        // SAFETY: All handles remain owned for the duration of this wait. The
        // vector has at most 63 entries, leaving the last slot for messages.
        let result = unsafe {
            MsgWaitForMultipleObjectsEx(
                handles.len() as u32,
                handles.as_ptr(),
                INFINITE,
                QS_ALLINPUT,
                MWMO_INPUTAVAILABLE,
            )
        };
        if result == WAIT_FAILED {
            break;
        }
        let index = result.wrapping_sub(WAIT_OBJECT_0) as usize;
        if control.stopping.load(Ordering::Acquire) {
            return;
        }
        let mut dirty = index <= processes.len();
        if index > processes.len()
            && let Some(&(directory, is_root)) = directory_handles.get(index - processes.len() - 1)
        {
            dirty |= directories[directory].changed(is_root);
        }
        // SAFETY: The message storage is local and dispatch is on the queue's
        // owner thread, where the out-of-context hook callback is registered.
        while unsafe { PeekMessageW(&mut message, null_mut(), 0, 0, PM_REMOVE) } != 0 {
            if message.message == WM_QUIT {
                return;
            }
            // SAFETY: The message was dequeued on this same thread.
            unsafe { DispatchMessageW(&message) };
        }
        if dirty | DIRTY.with(|state| state.replace(false)) {
            processes = refresh(&identities, &sender);
        }
    }
}

struct WindowHook(HWINEVENTHOOK);

impl WindowHook {
    fn register(first: u32, last: u32) -> Option<Self> {
        // SAFETY: Out-of-context hooks execute this static callback on the
        // registering worker. It does not retain any OS-owned pointer.
        let hook = unsafe {
            SetWinEventHook(
                first,
                last,
                null_mut(),
                Some(window_event),
                0,
                0,
                WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
            )
        };
        (!hook.is_null()).then(|| Self(hook))
    }
}

impl Drop for WindowHook {
    fn drop(&mut self) {
        // SAFETY: This is the hook returned by the current thread's registration.
        unsafe { UnhookWinEvent(self.0) };
    }
}

unsafe extern "system" fn window_event(
    _: HWINEVENTHOOK,
    event: u32,
    window: HWND,
    object: i32,
    child: i32,
    _: u32,
    _: u32,
) {
    if window.is_null() || object != OBJID_WINDOW || child != 0 {
        return;
    }
    if event == EVENT_OBJECT_DESTROY {
        if UNITY_WINDOWS.with(|known| known.borrow_mut().remove(&(window as usize))) {
            DIRTY.with(|state| state.set(true));
        }
        return;
    }
    // SAFETY: These APIs query an OS-owned window and tolerate a stale handle.
    if unsafe { GetAncestor(window, GA_ROOT) } != window {
        return;
    }
    let tracked = UNITY_WINDOWS.with(|known| known.borrow().contains(&(window as usize)));
    if tracked || is_unity_process_name(window_pid(window)) {
        UNITY_WINDOWS.with(|known| {
            let mut known = known.borrow_mut();
            if known.len() < WINDOW_LIMIT {
                known.insert(window as usize);
            }
        });
        DIRTY.with(|state| state.set(true));
    }
}

struct ChangeHandle(HANDLE);

impl ChangeHandle {
    fn new(path: &Path, filter: u32) -> Option<Self> {
        let wide: Vec<_> = path.as_os_str().encode_wide().chain([0]).collect();
        // SAFETY: The path is terminated and only borrowed during registration.
        let handle = unsafe { FindFirstChangeNotificationW(wide.as_ptr(), 0, filter) };
        (handle != INVALID_HANDLE_VALUE && !handle.is_null()).then(|| Self(handle))
    }

    fn rearm(&self) -> bool {
        // SAFETY: The change notification is still owned by this worker.
        unsafe { FindNextChangeNotification(self.0) != 0 }
    }
}

impl Drop for ChangeHandle {
    fn drop(&mut self) {
        // SAFETY: Change notifications must be closed using their own API.
        unsafe { FindCloseChangeNotification(self.0) };
    }
}

struct DirectoryWatch {
    temp_path: PathBuf,
    root: Option<ChangeHandle>,
    temp: Option<ChangeHandle>,
    lock_present: bool,
}

impl DirectoryWatch {
    fn new(root: &Path) -> Self {
        let temp_path = root.join("Temp");
        Self {
            root: ChangeHandle::new(root, FILE_NOTIFY_CHANGE_DIR_NAME),
            temp: ChangeHandle::new(&temp_path, FILE_NOTIFY_CHANGE_FILE_NAME),
            lock_present: temp_path.join("UnityLockFile").exists(),
            temp_path,
        }
    }

    fn changed(&mut self, is_root: bool) -> bool {
        if is_root {
            if self.root.as_ref().is_some_and(|handle| !handle.rearm()) {
                self.root = None;
            }
            self.temp = ChangeHandle::new(&self.temp_path, FILE_NOTIFY_CHANGE_FILE_NAME);
        } else if self.temp.as_ref().is_some_and(|handle| !handle.rearm()) {
            self.temp = None;
        }
        // Presence is an invalidation hint only. State comes from the verified
        // process and window, so stale locks never establish an active Editor.
        let present = self.temp_path.join("UnityLockFile").exists();
        std::mem::replace(&mut self.lock_present, present) != present
    }
}

struct Editor {
    pid: u32,
    identity: Vec<u8>,
    process: OwnedHandle,
}

fn open_process(pid: u32, access: u32) -> Option<OwnedHandle> {
    // SAFETY: A non-null successful handle is owned by the caller and closed
    // exactly once by OwnedHandle.
    let handle = unsafe { OpenProcess(access, 0, pid) };
    (!handle.is_null()).then(|| {
        // SAFETY: The non-null handle returned by OpenProcess is owned.
        unsafe { OwnedHandle::from_raw_handle(handle) }
    })
}

fn is_unity_process_name(pid: u32) -> bool {
    let Some(process) = open_process(pid, PROCESS_QUERY_LIMITED_INFORMATION) else {
        return false;
    };
    let mut image = [0; 1024];
    let mut length = image.len() as u32;
    // SAFETY: The buffer has the reported capacity, and the process handle is valid.
    let success = unsafe {
        QueryFullProcessImageNameW(process.as_raw_handle(), 0, image.as_mut_ptr(), &mut length)
    };
    success != 0
        && String::from_utf16_lossy(&image[..length as usize])
            .rsplit('\\')
            .next()
            .is_some_and(|name| name.eq_ignore_ascii_case("unity.exe"))
}

fn project_argument(process: &ProcessEvidence) -> Option<&Path> {
    let arguments = process.arguments()?;
    if arguments
        .iter()
        .any(|arg| arg.eq_ignore_ascii_case("-batchmode"))
    {
        return None;
    }
    let mut selectors = arguments
        .windows(2)
        .filter(|pair| pair[0].eq_ignore_ascii_case("-projectPath"));
    let project = Path::new(&selectors.next()?[1]);
    (project.is_absolute() && selectors.next().is_none()).then_some(project)
}

fn same_process(process: &OwnedHandle, start: u64) -> bool {
    let (mut created, mut exited, mut kernel, mut user) = (
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
    );
    // SAFETY: All outputs are writable FILETIME values and the handle is owned.
    let ok = unsafe {
        GetProcessTimes(
            process.as_raw_handle(),
            &mut created,
            &mut exited,
            &mut kernel,
            &mut user,
        )
    };
    let created = u64::from(created.dwHighDateTime) << 32 | u64::from(created.dwLowDateTime);
    ok != 0 && created.saturating_sub(116_444_736_000_000_000) / 10_000_000 == start
}

fn collect_editors(identities: &[Option<Vec<u8>>]) -> Vec<Editor> {
    observe_processes()
        .processes()
        .iter()
        .filter_map(|item| {
            if !item.name().eq_ignore_ascii_case("unity.exe")
                && !item.name().eq_ignore_ascii_case("unity")
            {
                return None;
            }
            let identity = file_identity_key(project_argument(item)?).ok()?;
            if !identities
                .iter()
                .any(|target| target.as_ref() == Some(&identity))
            {
                return None;
            }
            inspect_unity_installation(item.executable()?).ok()?;
            let process =
                open_process(item.pid(), SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION)?;
            (same_process(&process, item.start_time()) && active(&process)).then_some(Editor {
                pid: item.pid(),
                identity,
                process,
            })
        })
        .collect()
}

fn window_pid(window: HWND) -> u32 {
    let mut pid = 0;
    // SAFETY: The output is writable and the API tolerates a stale HWND.
    unsafe { GetWindowThreadProcessId(window, &mut pid) };
    pid
}

fn is_ready_window(window: HWND) -> bool {
    let mut class = [0; 64];
    // SAFETY: These synchronous queries borrow an OS-owned window and a local
    // buffer. Unity's interactive main container has a visible menu.
    let length = unsafe {
        if IsWindowVisible(window) == 0 || GetMenu(window).is_null() {
            return false;
        }
        GetClassNameW(window, class.as_mut_ptr(), class.len() as i32)
    };
    length > 0 && String::from_utf16_lossy(&class[..length as usize]) == "UnityContainerWndClass"
}

#[derive(Default)]
struct Windows {
    known_pids: BTreeSet<u32>,
    known_windows: BTreeSet<usize>,
    ready: Vec<(u32, HWND)>,
}

unsafe extern "system" fn enumerate_window(window: HWND, context: isize) -> i32 {
    // SAFETY: EnumWindows calls synchronously with the unique pointer supplied by windows().
    let windows = unsafe { &mut *(context as *mut Windows) };
    let pid = window_pid(window);
    if windows.known_pids.contains(&pid) {
        if windows.known_windows.len() < WINDOW_LIMIT {
            windows.known_windows.insert(window as usize);
        }
        if is_ready_window(window) {
            windows.ready.push((pid, window));
        }
    }
    1
}

fn windows(editors: &[Editor]) -> io::Result<Windows> {
    let mut result = Windows {
        known_pids: editors.iter().map(|editor| editor.pid).collect(),
        ..Windows::default()
    };
    // SAFETY: The callback borrows result only within this synchronous call.
    let enumerated = unsafe {
        EnumWindows(
            Some(enumerate_window),
            (&mut result as *mut Windows) as isize,
        )
    };
    if enumerated == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(result)
    }
}

fn active(process: &OwnedHandle) -> bool {
    // SAFETY: The owned process was opened with SYNCHRONIZE access.
    unsafe { WaitForSingleObject(process.as_raw_handle(), 0) == WAIT_TIMEOUT }
}

fn project_state(has_editor: bool, ready_count: usize, watches_exit: bool) -> UnityWindowState {
    match (has_editor, ready_count, watches_exit) {
        (false, _, _) => UnityWindowState::Closed,
        (true, 0, true) => UnityWindowState::Opening,
        (true, 0, false) => UnityWindowState::Unavailable,
        (true, 1, _) => UnityWindowState::Ready,
        (true, _, _) => UnityWindowState::Open,
    }
}

fn refresh(
    identities: &[Option<Vec<u8>>],
    sender: &watch::Sender<UnityWindowSnapshot>,
) -> Vec<OwnedHandle> {
    let editors = collect_editors(identities);
    let windows = windows(&editors).ok();
    if let Some(windows) = &windows {
        UNITY_WINDOWS.with(|known| known.replace(windows.known_windows.clone()));
    }
    let states = identities
        .iter()
        .map(|identity| {
            let (Some(identity), Some(windows)) = (identity, &windows) else {
                return UnityWindowState::Unavailable;
            };
            let matching: BTreeSet<_> = editors
                .iter()
                .filter(|editor| &editor.identity == identity && active(&editor.process))
                .map(|editor| editor.pid)
                .collect();
            project_state(
                !matching.is_empty(),
                windows
                    .ready
                    .iter()
                    .filter(|(pid, _)| matching.contains(pid))
                    .count(),
                editors
                    .iter()
                    .take(PROCESS_WAIT_LIMIT)
                    .any(|editor| matching.contains(&editor.pid)),
            )
        })
        .collect::<Vec<_>>();
    sender.send_if_modified(|current| {
        if current.states == states {
            return false;
        }
        current.version = current.version.wrapping_add(1);
        current.states = states;
        true
    });
    editors
        .into_iter()
        .take(PROCESS_WAIT_LIMIT)
        .map(|editor| editor.process)
        .collect()
}

pub fn focus_unity_window(root: &Path) -> io::Result<UnityWindowFocus> {
    let identity = file_identity_key(root)?;
    let editors = collect_editors(&[Some(identity.clone())]);
    let windows = windows(&editors)?;
    let [(pid, window)] = windows.ready.as_slice() else {
        return Ok(UnityWindowFocus::WindowNotFound);
    };
    // Hold the candidate's process handle while reading its current selector
    // again. A stale process/window snapshot must never activate another target.
    let valid = observe_processes().processes().iter().any(|process| {
        process.pid() == *pid
            && editors.iter().any(|editor| {
                editor.pid == *pid
                    && same_process(&editor.process, process.start_time())
                    && active(&editor.process)
            })
            && project_argument(process)
                .and_then(|path| file_identity_key(path).ok())
                .as_ref()
                == Some(&identity)
    });
    if !valid || window_pid(*window) != *pid || !is_ready_window(*window) {
        return Ok(UnityWindowFocus::WindowNotFound);
    }
    // SAFETY: The HWND was rechecked against the selected live process. No
    // input is injected and Windows retains the final activation decision.
    unsafe {
        if IsIconic(*window) != 0 {
            ShowWindowAsync(*window, SW_RESTORE);
        }
        if GetForegroundWindow() == *window || SetForegroundWindow(*window) != 0 {
            Ok(UnityWindowFocus::Activated)
        } else {
            Ok(UnityWindowFocus::Rejected)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_process_is_opening_until_one_matching_main_window_is_ready() {
        assert_eq!(project_state(false, 0, false), UnityWindowState::Closed);
        assert_eq!(project_state(true, 0, true), UnityWindowState::Opening);
        assert_eq!(project_state(true, 0, false), UnityWindowState::Unavailable);
        assert_eq!(project_state(true, 1, false), UnityWindowState::Ready);
        assert_eq!(project_state(true, 2, true), UnityWindowState::Open);
    }
}
