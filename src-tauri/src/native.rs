#[cfg(windows)]
mod windows {
    use std::sync::OnceLock;
    use windows_sys::Win32::{
        Foundation::*,
        System::{
            DataExchange::GetClipboardSequenceNumber, LibraryLoader::GetModuleHandleW,
            RemoteDesktop::*,
        },
        UI::WindowsAndMessaging::*,
    };
    static APP: OnceLock<tauri::AppHandle> = OnceLock::new();
    unsafe extern "system" fn procedure(hwnd: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
        if (message == WM_WTSSESSION_CHANGE && w == WTS_SESSION_LOCK as usize)
            || (message == WM_POWERBROADCAST && w == PBT_APMSUSPEND as usize)
        {
            if let Some(app) = APP.get() {
                crate::lock_app(app);
            }
        }
        DefWindowProcW(hwnd, message, w, l)
    }
    pub fn sequence() -> u32 {
        unsafe { GetClipboardSequenceNumber() }
    }
    pub fn listen(app: tauri::AppHandle) -> Result<(), String> {
        APP.set(app).map_err(|_| "重复的系统事件监听".to_string())?;
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        std::thread::spawn(move || unsafe {
            let name: Vec<u16> = "KeyKeeperSessionListener\0".encode_utf16().collect();
            let instance = GetModuleHandleW(std::ptr::null());
            let class = WNDCLASSW {
                lpfnWndProc: Some(procedure),
                hInstance: instance,
                lpszClassName: name.as_ptr(),
                ..std::mem::zeroed()
            };
            if RegisterClassW(&class) == 0 {
                let _ = tx.send(Err("无法注册系统事件监听".into()));
                return;
            }
            let hwnd = CreateWindowExW(
                0,
                name.as_ptr(),
                name.as_ptr(),
                0,
                0,
                0,
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                instance,
                std::ptr::null(),
            );
            if hwnd.is_null() || WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION) == 0
            {
                let _ = tx.send(Err("无法监听 Windows 锁屏事件".into()));
                return;
            }
            let _ = tx.send(Ok(()));
            let mut message = std::mem::zeroed();
            while GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
            WTSUnRegisterSessionNotification(hwnd);
            DestroyWindow(hwnd);
        });
        rx.recv().map_err(|_| "系统事件监听启动失败".to_string())?
    }
}

pub fn clipboard_sequence() -> u32 {
    #[cfg(windows)]
    {
        windows::sequence()
    }
    #[cfg(not(windows))]
    {
        0
    }
}
pub fn listen(app: tauri::AppHandle) -> Result<(), String> {
    #[cfg(windows)]
    {
        windows::listen(app)
    }
    #[cfg(not(windows))]
    {
        let _ = app;
        Ok(())
    }
}
