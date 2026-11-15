mod core;
mod native;

use serde_json::{json, Value};
use std::{
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::{Emitter, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_dialog::DialogExt;
use zeroize::Zeroizing;

pub(crate) struct ClipboardLease {
    text: Zeroizing<String>,
    until: Instant,
    sequence: u32,
}

pub(crate) struct AppState {
    core: Mutex<core::Core>,
    clipboard: Mutex<Option<ClipboardLease>>,
}

fn clear_clipboard(app: &tauri::AppHandle, force: bool) {
    let state = app.state::<AppState>();
    let Ok(mut guard) = state.clipboard.lock() else {
        return;
    };
    let Some(lease) = guard.as_ref() else { return };
    if !force && Instant::now() < lease.until {
        return;
    }
    if native::clipboard_sequence() == lease.sequence {
        if let Ok(text) = app.clipboard().read_text() {
            if text == *lease.text {
                let _ = app.clipboard().clear();
            }
        }
    }
    *guard = None;
}

pub(crate) fn lock_app(app: &tauri::AppHandle) {
    let state = app.state::<AppState>();
    if let Ok(mut core) = state.core.lock() {
        core.lock();
        clear_clipboard(app, true);
        let _ = app.emit("vault-locked", ());
    };
}

fn run_request(app: tauri::AppHandle, mut request: Value) -> Result<Value, String> {
    let op = request
        .get("op")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let state = app.state::<AppState>();
    // Paths are supplied exclusively by native dialogs, never by web content.
    if let Some(object) = request.as_object_mut() {
        object.remove("path");
    }
    if matches!(op.as_str(), "import_open" | "export_backup") {
        let mut core = state.core.lock().map_err(|_| "保险箱暂时不可用")?;
        if core.expire() {
            clear_clipboard(&app, true);
            let _ = app.emit("vault-locked", ());
        }
        let status = core.execute(json!({"op":"status"}))?;
        if status["session"].is_null() || status["session"] != request["session"] {
            return Err("保险箱已锁定，请重新解锁".into());
        }
    }
    match op.as_str() {
        "import_open" | "inspect_backup" | "export_backup" => {
            let dialog = app.dialog().file();
            let path = match op.as_str() {
                "import_open" => dialog
                    .add_filter("账户资料", &["txt", "csv", "xls", "xlsx"])
                    .blocking_pick_file(),
                "inspect_backup" => dialog
                    .add_filter("加密保险箱", &["kkv"])
                    .blocking_pick_file(),
                _ => dialog
                    .add_filter("加密保险箱", &["kkv"])
                    .set_file_name("Key-Keeper-backup.kkv")
                    .blocking_save_file(),
            };
            let Some(path) = path else {
                return Ok(json!({"cancelled":true}));
            };
            let path = path.into_path().map_err(|_| "无法访问所选文件")?;
            request["path"] = json!(path.to_string_lossy());
        }
        "copy" => {
            let mut core = state.core.lock().map_err(|_| "保险箱暂时不可用")?;
            if core.expire() {
                clear_clipboard(&app, true);
                let _ = app.emit("vault-locked", ());
            }
            let value = core.copy_value(&request)?;
            let mut lease = state.clipboard.lock().map_err(|_| "剪贴板暂时不可用")?;
            app.clipboard()
                .write_text(value.as_str())
                .map_err(|_| "无法写入剪贴板")?;
            *lease = Some(ClipboardLease {
                text: value,
                until: Instant::now() + Duration::from_secs(30),
                sequence: native::clipboard_sequence(),
            });
            return Ok(Value::Null);
        }
        "lock" => {
            lock_app(&app);
            return Ok(Value::Null);
        }
        _ => {}
    }
    let mut core = state.core.lock().map_err(|_| "保险箱暂时不可用")?;
    if core.expire() {
        clear_clipboard(&app, true);
        let _ = app.emit("vault-locked", ());
    }
    let result = core.execute(request);
    if result.is_ok() && matches!(op.as_str(), "restore_backup" | "change_password") {
        clear_clipboard(&app, true);
    }
    result
}

#[tauri::command]
async fn rpc(app: tauri::AppHandle, request: Value) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || run_request(app, request))
        .await
        .map_err(|_| "操作未完成，请重试")?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            let data = app.path().app_data_dir()?;
            #[cfg(debug_assertions)]
            let data = std::env::var_os("KEY_KEEPER_DATA_DIR")
                .map(std::path::PathBuf::from)
                .unwrap_or(data);
            app.manage(AppState {
                core: Mutex::new(core::Core::new(data)),
                clipboard: Mutex::new(None),
            });
            native::listen(app.handle().clone())?;
            let handle = app.handle().clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(Duration::from_secs(1));
                let state = handle.state::<AppState>();
                if let Ok(mut core) = state.core.lock() {
                    let expired = core.expire();
                    clear_clipboard(&handle, expired);
                    if expired {
                        let _ = handle.emit("vault-locked", ());
                    }
                };
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![rpc])
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
                lock_app(window.app_handle());
            }
        })
        .run(tauri::generate_context!())
        .expect("无法启动 Key Keeper");
}
