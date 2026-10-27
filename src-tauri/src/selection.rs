//! Clipboard-first capture. The target application performs copying; this module
//! never clears, restores or writes clipboard data and never invokes a model.
use serde::Serialize;
use std::{
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
        mpsc::{SyncSender, sync_channel},
    },
    time::Instant,
};
use tauri::{Emitter, Manager};
#[cfg(target_os = "windows")]
mod native;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CapturedText {
    pub request_id: String,
    pub text: String,
    pub elapsed_ms: u64,
}
#[derive(Clone, Debug, Serialize)]
pub(crate) struct CaptureError {
    pub code: &'static str,
    pub message: String,
}
fn error(code: &'static str, message: &str) -> CaptureError {
    CaptureError {
        code,
        message: message.into(),
    }
}
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CaptureView {
    pub revision: u64,
    pub busy: bool,
    pub latest: Option<CapturedText>,
    pub error: Option<CaptureError>,
    pub hotkey_error: Option<String>,
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct Target {
    hwnd: isize,
    pid: u32,
}
struct Request {
    id: String,
    target: Target,
    started: Instant,
}
impl CaptureView {
    fn begin(&mut self, target: Target) -> Option<Request> {
        self.revision += 1;
        if self.busy {
            self.error = Some(error("busy", "正在读取选中文字，请稍后再试。"));
            return None;
        }
        self.busy = true;
        self.latest = None;
        self.error = None;
        Some(Request {
            id: format!("selection-{}-{}", std::process::id(), self.revision),
            target,
            started: Instant::now(),
        })
    }
    fn finish(&mut self, result: Result<CapturedText, CaptureError>) {
        self.busy = false;
        self.revision += 1;
        match result {
            Ok(text) => {
                self.latest = Some(text);
                self.error = None;
            }
            Err(e) => {
                self.latest = None;
                self.error = Some(e);
            }
        }
    }
}
#[derive(Default)]
pub(crate) struct CaptureState {
    view: Mutex<CaptureView>,
    sender: OnceLock<SyncSender<Request>>,
    pub stopping: AtomicBool,
}
pub(crate) fn target_window() -> Option<Target> {
    #[cfg(target_os = "windows")]
    {
        native::target_window()
    }
    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}
fn notify(app: &tauri::AppHandle) {
    let _ = app.emit("selection-state-changed", ());
}
impl CaptureState {
    pub fn start(&self, app: tauri::AppHandle) -> Result<(), String> {
        let (sender, receiver) = sync_channel::<Request>(1);
        std::thread::Builder::new()
            .name("clipboard-selection".into())
            .spawn(move || {
                while let Ok(request) = receiver.recv() {
                    let state = app.state::<CaptureState>();
                    let result = std::panic::catch_unwind(|| {
                        if state.stopping.load(Ordering::Acquire) {
                            return Err(error("cancelled", "应用正在退出，本次取词已取消。"));
                        }
                        if request.started.elapsed().as_millis() > 2000 {
                            return Err(error("timeout", "取词任务已过期，请重新触发。"));
                        }
                        #[cfg(target_os = "windows")]
                        {
                            native::capture(&request)
                        }
                        #[cfg(not(target_os = "windows"))]
                        {
                            Err(error(
                                "unsupported",
                                "当前平台不支持剪贴板取词，请手动粘贴。",
                            ))
                        }
                    })
                    .unwrap_or_else(|_| {
                        Err(error("capture_failed", "读取选中文字失败，请手动粘贴。"))
                    });
                    let activate = result
                        .as_ref()
                        .err()
                        .is_none_or(|e| !matches!(e.code, "focus_changed" | "cancelled"));
                    if let Ok(mut view) = state.view.lock() {
                        view.finish(result);
                    }
                    notify(&app);
                    if state.stopping.load(Ordering::Acquire) {
                        app.exit(0);
                        continue;
                    }
                    // Capture has ended; activation cannot destroy the source selection midway.
                    if activate && let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.unminimize();
                        let _ = window.set_focus();
                    }
                }
            })
            .map_err(|_| "无法启动剪贴板取词线程。".to_string())?;
        self.sender
            .set(sender)
            .map_err(|_| "取词线程已经启动。".to_string())
    }
    pub fn submit(&self, app: &tauri::AppHandle, target: Target) {
        if self.stopping.load(Ordering::Acquire) {
            return;
        }
        let request = self.view.lock().ok().and_then(|mut view| {
            if self.stopping.load(Ordering::Acquire) {
                None
            } else {
                view.begin(target)
            }
        });
        if let Some(request) = request
            && self
                .sender
                .get()
                .is_none_or(|sender| sender.try_send(request).is_err())
            && let Ok(mut view) = self.view.lock()
        {
            view.finish(Err(error("unavailable", "取词服务不可用，请手动粘贴。")));
        }
        notify(app);
    }
    pub fn busy(&self) -> bool {
        self.view.lock().map(|v| v.busy).unwrap_or(true)
    }
    pub fn request_exit(&self) -> bool {
        let Ok(view) = self.view.lock() else {
            return false;
        };
        self.stopping.store(true, Ordering::Release);
        view.busy
    }
    pub fn set_hotkey_error(&self, app: &tauri::AppHandle, message: Option<String>) {
        if let Ok(mut view) = self.view.lock() {
            view.hotkey_error = message;
            view.revision += 1;
        }
        notify(app);
    }
}
#[tauri::command]
pub(crate) fn get_selection_state(
    state: tauri::State<CaptureState>,
) -> Result<CaptureView, String> {
    state
        .view
        .lock()
        .map(|view| view.clone())
        .map_err(|_| "取词状态不可用。".into())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn one_transaction_rejects_repeated_input_and_recovers_after_failure() {
        let mut view = CaptureView::default();
        let target = Target { hwnd: 1, pid: 2 };
        let first = view.begin(target).unwrap();
        for _ in 0..10 {
            assert!(view.begin(target).is_none());
        }
        assert!(view.busy);
        view.finish(Err(error("timeout", "timeout")));
        assert!(!view.busy);
        assert_ne!(first.id, view.begin(target).unwrap().id);
        view.finish(Ok(CapturedText {
            request_id: "b".into(),
            text: "B".into(),
            elapsed_ms: 1,
        }));
        assert_eq!(view.latest.as_ref().unwrap().text, "B");
        view.begin(target);
        assert!(view.latest.is_none());
    }
}
