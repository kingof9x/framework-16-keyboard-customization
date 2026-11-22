//! Windows-only parts: the "Open an app" keys, the icon in the notification area
//! (system tray), starting with Windows, and the file picker.
//!
//! "Open an app" keys send Ctrl+Shift+Alt+F13 to F24 (see `keycodes::app_code`). A
//! background thread registers those as system-wide hotkeys and starts the chosen
//! program when one arrives. The same thread owns the tray icon.

/// What the tray icon asks the window to do.
#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayEvent {
    Open,
    Quit,
    /// Ctrl+Alt+Shift+M: audio mode on or off (the same shortcut as White Rabbit's).
    AudioHotkey,
}

#[cfg(windows)]
pub use imp::*;

#[cfg(not(windows))]
pub use stub::*;

#[cfg(not(windows))]
mod stub {
    use super::TrayEvent;

    /// Off Windows there are no hotkeys or tray: keeps the app building for tests and the demo.
    pub struct Background;

    impl Background {
        pub fn start(_on_event: impl Fn(TrayEvent) + Send + 'static) -> Self {
            Background
        }
        pub fn set_apps(&self, _apps: Vec<String>) {}
        pub fn set_tray(&self, _on: bool) {}
        pub fn set_audio_hotkey(&self, _on: bool) {}
        pub fn close(&self) {}
    }

    pub fn set_autostart(_on: bool) {}

    pub fn browse_for_app() -> Option<String> {
        None
    }

    pub fn browse_for_firmware() -> Option<String> {
        None
    }

    pub fn open(_what: &str) -> bool {
        false
    }
}

#[cfg(windows)]
mod imp {
    use super::TrayEvent;
    use f16_core::keycodes::APP_SLOTS;
    use std::cell::RefCell;
    use std::sync::atomic::{AtomicIsize, Ordering};
    use std::sync::{Arc, Mutex};
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
        KEY_SET_VALUE, REG_SZ,
    };
    use windows_sys::Win32::UI::Controls::Dialogs::{
        GetOpenFileNameW, OFN_FILEMUSTEXIST, OFN_NOCHANGEDIR, OFN_PATHMUSTEXIST, OPENFILENAMEW,
    };
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, RegisterHotKey, UnregisterHotKey, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT,
        MOD_SHIFT, VK_CONTROL, VK_F13, VK_MENU, VK_SHIFT,
    };
    use windows_sys::Win32::UI::Shell::{
        ShellExecuteW, Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE,
        NOTIFYICONDATAW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    const WM_TRAY: u32 = WM_APP + 1;
    const WM_SYNC: u32 = WM_APP + 2;
    const WM_AUDIO_HOTKEY: u32 = WM_APP + 3;
    const VK_M: u32 = 0x4D;

    /// Where the keyboard hook posts the shortcut (the hidden window), and whether M is down.
    static HOOK_HWND: AtomicIsize = AtomicIsize::new(0);
    static M_DOWN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    const MENU_OPEN: usize = 1;
    const MENU_QUIT: usize = 2;
    const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
    const RUN_VALUE: &str = "Framework 16 Keyboard Control";

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    #[derive(Default)]
    struct Wanted {
        apps: Vec<String>,
        tray: bool,
        audio_hotkey: bool,
    }

    struct ThreadState {
        wanted: Arc<Mutex<Wanted>>,
        on_event: Box<dyn Fn(TrayEvent)>,
        apps: Vec<String>,
        tray_shown: bool,
        icon: HICON,
        hook: HHOOK,
    }

    thread_local! {
        static STATE: RefCell<Option<ThreadState>> = const { RefCell::new(None) };
    }

    /// The background thread with the hotkeys and the tray icon.
    pub struct Background {
        wanted: Arc<Mutex<Wanted>>,
        hwnd: Arc<AtomicIsize>,
    }

    impl Background {
        pub fn start(on_event: impl Fn(TrayEvent) + Send + 'static) -> Self {
            let wanted = Arc::new(Mutex::new(Wanted::default()));
            let hwnd = Arc::new(AtomicIsize::new(0));
            let (w, h) = (wanted.clone(), hwnd.clone());
            let _ = std::thread::Builder::new()
                .name("hotkeys-tray".into())
                .spawn(move || run(w, h, Box::new(on_event)));
            Self { wanted, hwnd }
        }

        fn sync(&self) {
            let h = self.hwnd.load(Ordering::Acquire);
            if h != 0 {
                unsafe { PostMessageW(h as HWND, WM_SYNC, 0, 0) };
            }
        }

        /// Program paths for the "Open an app" keys (empty = not used).
        pub fn set_apps(&self, apps: Vec<String>) {
            self.wanted.lock().unwrap().apps = apps;
            self.sync();
        }

        pub fn set_tray(&self, on: bool) {
            self.wanted.lock().unwrap().tray = on;
            self.sync();
        }

        /// Watch for Ctrl+Alt+Shift+M (audio mode). It is watched rather than registered as a
        /// hotkey, so White Rabbit, which registers the same shortcut, still gets it too.
        pub fn set_audio_hotkey(&self, on: bool) {
            self.wanted.lock().unwrap().audio_hotkey = on;
            self.sync();
        }

        /// Removes the tray icon and the hotkeys before the app exits.
        pub fn close(&self) {
            let h = self.hwnd.swap(0, Ordering::AcqRel);
            if h != 0 {
                unsafe { SendMessageW(h as HWND, WM_CLOSE, 0, 0) };
            }
        }
    }

    fn run(
        wanted: Arc<Mutex<Wanted>>,
        hwnd_out: Arc<AtomicIsize>,
        on_event: Box<dyn Fn(TrayEvent)>,
    ) {
        unsafe {
            let hinst = GetModuleHandleW(std::ptr::null());
            let class = wide("F16KeyboardControlTray");
            let wc = WNDCLASSW {
                lpfnWndProc: Some(wndproc),
                hInstance: hinst,
                lpszClassName: class.as_ptr(),
                ..std::mem::zeroed()
            };
            RegisterClassW(&wc);
            let title = wide("Framework 16 Keyboard Control");
            let hwnd = CreateWindowExW(
                0,
                class.as_ptr(),
                title.as_ptr(),
                WS_OVERLAPPED,
                0,
                0,
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                hinst,
                std::ptr::null(),
            );
            if hwnd.is_null() {
                return;
            }
            STATE.with(|s| {
                *s.borrow_mut() = Some(ThreadState {
                    wanted,
                    on_event,
                    apps: vec![String::new(); APP_SLOTS],
                    tray_shown: false,
                    icon: app_icon(),
                    hook: std::ptr::null_mut(),
                })
            });
            hwnd_out.store(hwnd as isize, Ordering::Release);
            HOOK_HWND.store(hwnd as isize, Ordering::Release);
            sync(hwnd);
            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }

    fn tray_data(hwnd: HWND, icon: HICON) -> NOTIFYICONDATAW {
        let mut nid: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = 1;
        nid.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
        nid.uCallbackMessage = WM_TRAY;
        nid.hIcon = icon;
        let tip: Vec<u16> = "Framework 16 Keyboard Control".encode_utf16().collect();
        nid.szTip[..tip.len()].copy_from_slice(&tip);
        nid
    }

    /// Brings the hotkeys and the tray icon in line with what the window wants.
    /// The Windows calls happen after the state is released, so a message they
    /// send back to `wndproc` can't find it borrowed.
    fn sync(hwnd: HWND) {
        let plan = STATE.with(|s| {
            let mut s = s.try_borrow_mut().ok()?;
            let st = s.as_mut()?;
            let (apps, tray, audio_hotkey) = {
                let w = st.wanted.lock().unwrap();
                let mut apps = w.apps.clone();
                apps.resize(APP_SLOTS, String::new());
                (apps, w.tray, w.audio_hotkey)
            };
            // The keyboard hook is set up and removed right here: neither call sends
            // messages back to this window.
            if audio_hotkey && st.hook.is_null() {
                st.hook = unsafe {
                    SetWindowsHookExW(
                        WH_KEYBOARD_LL,
                        Some(keyboard_hook),
                        GetModuleHandleW(std::ptr::null()),
                        0,
                    )
                };
            } else if !audio_hotkey && !st.hook.is_null() {
                unsafe { UnhookWindowsHookEx(st.hook) };
                st.hook = std::ptr::null_mut();
            }
            let hotkeys: Vec<(usize, bool)> = st
                .apps
                .iter()
                .zip(&apps)
                .enumerate()
                .filter(|(_, (old, new))| old.is_empty() != new.is_empty())
                .map(|(i, (_, new))| (i, !new.is_empty()))
                .collect();
            st.apps = apps;
            let tray_change = (tray != st.tray_shown).then_some(tray);
            st.tray_shown = tray;
            Some((hotkeys, tray_change, st.icon))
        });
        let Some((hotkeys, tray_change, icon)) = plan else {
            return;
        };
        for (i, on) in hotkeys {
            unsafe {
                if on {
                    RegisterHotKey(
                        hwnd,
                        i as i32 + 1,
                        MOD_CONTROL | MOD_SHIFT | MOD_ALT | MOD_NOREPEAT,
                        (VK_F13 + i as u16) as u32,
                    );
                } else {
                    UnregisterHotKey(hwnd, i as i32 + 1);
                }
            }
        }
        if let Some(on) = tray_change {
            let nid = tray_data(hwnd, icon);
            unsafe { Shell_NotifyIconW(if on { NIM_ADD } else { NIM_DELETE }, &nid) };
        }
    }

    fn send(e: TrayEvent) {
        STATE.with(|s| {
            if let Ok(s) = s.try_borrow() {
                if let Some(st) = s.as_ref() {
                    (st.on_event)(e);
                }
            }
        });
    }

    /// Opens a program, file or web page the way Explorer would.
    pub fn open(what: &str) -> bool {
        launch(what);
        true
    }

    fn launch(path: &str) {
        let verb = wide("open");
        let file = wide(path);
        unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                verb.as_ptr(),
                file.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                SW_SHOWNORMAL,
            );
        }
    }

    unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        match msg {
            WM_SYNC => {
                sync(hwnd);
                0
            }
            WM_AUDIO_HOTKEY => {
                send(TrayEvent::AudioHotkey);
                0
            }
            WM_HOTKEY => {
                let slot = wp.wrapping_sub(1);
                let path = STATE.with(|s| {
                    s.try_borrow()
                        .ok()
                        .and_then(|s| s.as_ref().and_then(|st| st.apps.get(slot).cloned()))
                        .unwrap_or_default()
                });
                let path = path.trim().trim_matches('"');
                if !path.is_empty() {
                    launch(path);
                }
                0
            }
            WM_TRAY => {
                match (lp & 0xFFFF) as u32 {
                    WM_LBUTTONUP | WM_LBUTTONDBLCLK => send(TrayEvent::Open),
                    WM_RBUTTONUP | WM_CONTEXTMENU => unsafe { show_menu(hwnd) },
                    _ => {}
                }
                0
            }
            WM_COMMAND => {
                match wp & 0xFFFF {
                    MENU_OPEN => send(TrayEvent::Open),
                    MENU_QUIT => send(TrayEvent::Quit),
                    _ => {}
                }
                0
            }
            WM_DESTROY => {
                let shown = STATE.with(|s| {
                    s.try_borrow()
                        .ok()
                        .and_then(|s| s.as_ref().map(|st| (st.tray_shown, st.icon)))
                });
                if let Some((true, icon)) = shown {
                    let nid = tray_data(hwnd, icon);
                    unsafe { Shell_NotifyIconW(NIM_DELETE, &nid) };
                }
                unsafe { PostQuitMessage(0) };
                0
            }
            _ => unsafe { DefWindowProcW(hwnd, msg, wp, lp) },
        }
    }

    /// Sees every key press (without taking any): Ctrl+Alt+Shift+M toggles audio mode.
    unsafe extern "system" fn keyboard_hook(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        if code == HC_ACTION as i32 {
            let info = unsafe { &*(lp as *const KBDLLHOOKSTRUCT) };
            if info.vkCode == VK_M {
                let msg = wp as u32;
                if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
                    let held = |vk: u16| unsafe { GetAsyncKeyState(vk as i32) } < 0;
                    let first = !M_DOWN.swap(true, Ordering::AcqRel);
                    if first && held(VK_CONTROL) && held(VK_MENU) && held(VK_SHIFT) {
                        let h = HOOK_HWND.load(Ordering::Acquire);
                        if h != 0 {
                            unsafe { PostMessageW(h as HWND, WM_AUDIO_HOTKEY, 0, 0) };
                        }
                    }
                } else if msg == WM_KEYUP || msg == WM_SYSKEYUP {
                    M_DOWN.store(false, Ordering::Release);
                }
            }
        }
        unsafe { CallNextHookEx(std::ptr::null_mut(), code, wp, lp) }
    }

    unsafe fn show_menu(hwnd: HWND) {
        unsafe {
            let menu = CreatePopupMenu();
            let open = wide("Open Framework 16 Keyboard Control");
            let quit = wide("Quit");
            AppendMenuW(menu, MF_STRING, MENU_OPEN, open.as_ptr());
            AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
            AppendMenuW(menu, MF_STRING, MENU_QUIT, quit.as_ptr());
            let mut pt = POINT { x: 0, y: 0 };
            GetCursorPos(&mut pt);
            // Needed so the menu closes when you click elsewhere
            SetForegroundWindow(hwnd);
            TrackPopupMenu(menu, TPM_RIGHTBUTTON, pt.x, pt.y, 0, hwnd, std::ptr::null());
            PostMessageW(hwnd, WM_NULL, 0, 0);
            DestroyMenu(menu);
        }
    }

    /// The icon built into the .exe (resource 1), or a drawn one if it is missing.
    fn app_icon() -> HICON {
        let icon = unsafe {
            LoadImageW(
                GetModuleHandleW(std::ptr::null()),
                std::ptr::without_provenance::<u16>(1), // MAKEINTRESOURCE(1)
                IMAGE_ICON,
                GetSystemMetrics(SM_CXSMICON),
                GetSystemMetrics(SM_CYSMICON),
                LR_DEFAULTCOLOR,
            )
        };
        if icon.is_null() {
            make_icon()
        } else {
            icon as HICON
        }
    }

    /// A 32 x 32 icon of a small keyboard with a few lit keys, drawn in code.
    fn make_icon() -> HICON {
        const N: usize = 32;
        let mut bgra = vec![0u8; N * N * 4];
        let mut px = |x: usize, y: usize, c: [u8; 4]| {
            let i = (y * N + x) * 4;
            bgra[i..i + 4].copy_from_slice(&[c[2], c[1], c[0], c[3]]);
        };
        // body
        for y in 7..25 {
            for x in 1..31 {
                let edge = y == 7 || y == 24 || x == 1 || x == 30;
                px(
                    x,
                    y,
                    if edge {
                        [230, 230, 235, 255]
                    } else {
                        [40, 40, 46, 255]
                    },
                );
            }
        }
        // keys: three rows of small squares, a few in Framework orange and the accent blue
        for (row, y) in [(0, 10), (1, 15), (2, 20)] {
            for k in 0..6 {
                let x = 4 + k * 4;
                let c = match (row, k) {
                    (0, 1) | (1, 4) => [255, 92, 35, 255],
                    (2, 2) | (0, 4) => [96, 205, 255, 255],
                    _ => [200, 200, 208, 255],
                };
                for dy in 0..3 {
                    for dx in 0..3 {
                        px(x + dx, y + dy, c);
                    }
                }
            }
        }
        let and_mask = [0u8; N * N / 8];
        unsafe {
            CreateIcon(
                GetModuleHandleW(std::ptr::null()),
                N as i32,
                N as i32,
                1,
                32,
                and_mask.as_ptr(),
                bgra.as_ptr(),
            )
        }
    }

    /// Starts the app (hidden, in the tray) when you sign in to Windows, or stops doing so.
    pub fn set_autostart(on: bool) {
        unsafe {
            let mut key: HKEY = std::ptr::null_mut();
            let sub = wide(RUN_KEY);
            // The Run key always exists in a user's profile
            if RegOpenKeyExW(HKEY_CURRENT_USER, sub.as_ptr(), 0, KEY_SET_VALUE, &mut key) != 0 {
                return;
            }
            let name = wide(RUN_VALUE);
            if on {
                if let Ok(exe) = std::env::current_exe() {
                    let cmd = wide(&format!("\"{}\" --tray", exe.display()));
                    RegSetValueExW(
                        key,
                        name.as_ptr(),
                        0,
                        REG_SZ,
                        cmd.as_ptr() as *const u8,
                        (cmd.len() * 2) as u32,
                    );
                }
            } else {
                RegDeleteValueW(key, name.as_ptr());
            }
            RegCloseKey(key);
        }
    }

    /// Windows' Open dialog, for picking the program an "Open an app" key starts.
    pub fn browse_for_app() -> Option<String> {
        browse(
            "Programs\0*.exe;*.lnk;*.bat;*.cmd\0All files\0*.*\0\0",
            "Pick the program this key opens",
        )
    }

    pub fn browse_for_firmware() -> Option<String> {
        browse(
            "Firmware (.uf2)\0*.uf2\0All files\0*.*\0\0",
            "Pick the firmware file",
        )
    }

    /// The Windows "Open" dialog; `filter` is its double-NUL-ended filter list.
    fn browse(filter: &str, title: &str) -> Option<String> {
        let mut buf = vec![0u16; 1024];
        let filter: Vec<u16> = filter.encode_utf16().collect();
        let title = wide(title);
        let mut ofn: OPENFILENAMEW = unsafe { std::mem::zeroed() };
        ofn.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
        ofn.lpstrFilter = filter.as_ptr();
        ofn.lpstrFile = buf.as_mut_ptr();
        ofn.nMaxFile = buf.len() as u32;
        ofn.lpstrTitle = title.as_ptr();
        ofn.Flags = OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR;
        if unsafe { GetOpenFileNameW(&mut ofn) } == 0 {
            return None;
        }
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        Some(String::from_utf16_lossy(&buf[..len]))
    }
}
