use super::{CaptureError, CapturedText, Request, Target, error};
use std::{
    mem::size_of,
    thread,
    time::{Duration, Instant},
};
use windows::Win32::{
    Foundation::{HGLOBAL, HWND},
    System::{DataExchange::*, Memory::*},
    UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
};
const UNICODE: u32 = 13;
fn hwnd(target: Target) -> HWND {
    HWND(target.hwnd as *mut _)
}
fn process(window: HWND) -> u32 {
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(window, Some(&mut pid));
    }
    pid
}
pub(super) fn target_window() -> Option<Target> {
    let window = unsafe { GetForegroundWindow() };
    let pid = process(window);
    if window.0.is_null() || pid == 0 || pid == std::process::id() {
        return None;
    }
    Some(Target {
        hwnd: window.0 as isize,
        pid,
    })
}
fn check(target: Target) -> Result<(), CaptureError> {
    if unsafe { GetForegroundWindow() } != hwnd(target) || process(hwnd(target)) != target.pid {
        return Err(error(
            "focus_changed",
            "目标窗口已切换，本次取词已取消，请重新选中后触发。",
        ));
    }
    Ok(())
}
fn modifiers_down() -> bool {
    [VK_CONTROL, VK_MENU, VK_SHIFT, VK_LWIN, VK_RWIN]
        .iter()
        .any(|key| unsafe { GetAsyncKeyState(key.0 as i32) } < 0)
}
fn wait_release(target: Target) -> Result<(), CaptureError> {
    let deadline = Instant::now() + Duration::from_millis(800);
    while modifiers_down() {
        check(target)?;
        if Instant::now() >= deadline {
            return Err(error("modifier_timeout", "请松开快捷键后重新触发。"));
        }
        thread::sleep(Duration::from_millis(8));
    }
    check(target)
}
fn key(vk: VIRTUAL_KEY, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                dwFlags: flags,
                ..Default::default()
            },
        },
    }
}
fn copy(target: Target) -> Result<(), CaptureError> {
    check(target)?;
    if modifiers_down() {
        return Err(error(
            "input_changed",
            "按键状态发生变化，已取消复制，请重试。",
        ));
    }
    let events = [
        key(VK_CONTROL, KEYBD_EVENT_FLAGS(0)),
        key(VK_C, KEYBD_EVENT_FLAGS(0)),
        key(VK_C, KEYEVENTF_KEYUP),
        key(VK_CONTROL, KEYEVENTF_KEYUP),
    ];
    let count = unsafe { SendInput(&events, size_of::<INPUT>() as i32) };
    if count != 4 {
        if count > 0 {
            let cleanup = if count >= 2 {
                vec![key(VK_C, KEYEVENTF_KEYUP), key(VK_CONTROL, KEYEVENTF_KEYUP)]
            } else {
                vec![key(VK_CONTROL, KEYEVENTF_KEYUP)]
            };
            unsafe {
                SendInput(&cleanup, size_of::<INPUT>() as i32);
            }
        }
        return Err(error(
            "input_failed",
            "无法发送复制按键，可能存在权限或兼容性限制，请手动复制粘贴。",
        ));
    }
    Ok(())
}
struct ClipboardLock;
impl ClipboardLock {
    fn open(deadline: Instant, target: Target) -> Result<Self, CaptureError> {
        let deadline = deadline.min(Instant::now() + Duration::from_millis(300));
        loop {
            check(target)?;
            if unsafe { OpenClipboard(None) }.is_ok() {
                return Ok(Self);
            }
            if Instant::now() >= deadline {
                return Err(error(
                    "clipboard_busy",
                    "剪贴板被其他程序占用，请稍后重试。",
                ));
            }
            thread::sleep(Duration::from_millis(8));
        }
    }
}
impl Drop for ClipboardLock {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseClipboard();
        }
    }
}
struct LockedMemory(HGLOBAL);
impl Drop for LockedMemory {
    fn drop(&mut self) {
        unsafe {
            let _ = GlobalUnlock(self.0);
        }
    }
}
fn decode(units: &[u16]) -> Result<String, CaptureError> {
    let end = units
        .iter()
        .position(|v| *v == 0)
        .ok_or_else(|| error("invalid_text", "复制文本格式无效，请手动粘贴。"))?;
    let text = String::from_utf16(&units[..end])
        .map_err(|_| error("invalid_text", "复制文本编码无效，请重新选择。"))?;
    if text.trim().is_empty() {
        return Err(error("no_text", "未读取到文字，请先选中一段文本。"));
    }
    if text.chars().count() > 2000 {
        return Err(error(
            "too_long",
            "选中文字超过 2,000 字符，请缩短后重新取词。",
        ));
    }
    Ok(text)
}
fn read_text() -> Result<String, CaptureError> {
    if !unsafe { IsClipboardFormatAvailable(UNICODE) }.is_ok() {
        return Err(error("no_text", "复制结果没有文本，请选择可复制的文字。"));
    }
    let handle = unsafe { GetClipboardData(UNICODE) }
        .map_err(|_| error("read_failed", "无法读取复制文本，请手动粘贴。"))?;
    let global = HGLOBAL(handle.0);
    let size = unsafe { GlobalSize(global) };
    if size == 0 || size > 1_048_576 || size % 2 != 0 {
        return Err(error("too_large", "复制内容过大或格式无效，请缩短选区。"));
    }
    let ptr = unsafe { GlobalLock(global) };
    if ptr.is_null() {
        return Err(error("read_failed", "无法读取剪贴板，请稍后重试。"));
    }
    let _memory = LockedMemory(global);
    decode(unsafe { std::slice::from_raw_parts(ptr.cast::<u16>(), size / 2) })
}
pub(super) fn capture(request: &Request) -> Result<CapturedText, CaptureError> {
    let target = request.target;
    wait_release(target)?;
    let original = unsafe { GetClipboardSequenceNumber() };
    if original == 0 {
        return Err(error(
            "clipboard_unavailable",
            "无法访问系统剪贴板，请手动粘贴。",
        ));
    }
    copy(target)?;
    let deadline = Instant::now() + Duration::from_millis(800);
    let mut observed = original;
    let mut changed_at = Instant::now();
    while Instant::now() < deadline {
        check(target)?;
        let current = unsafe { GetClipboardSequenceNumber() };
        if current != observed {
            observed = current;
            changed_at = Instant::now();
        }
        if observed != original && changed_at.elapsed() >= Duration::from_millis(24) {
            let _lock = ClipboardLock::open(deadline, target)?;
            let current = unsafe { GetClipboardSequenceNumber() };
            if current != observed {
                observed = current;
                changed_at = Instant::now();
                continue;
            }
            // A sequence change alone is not proof of provenance. Reject observed
            // owners outside the original process; never substitute old clipboard text.
            let owner = unsafe { GetClipboardOwner() }
                .map_err(|_| error("ambiguous_copy", "无法确认复制来源，请手动粘贴。"))?;
            if process(owner) != target.pid {
                return Err(error(
                    "ambiguous_copy",
                    "剪贴板被其他应用更新，未采用其中的内容。",
                ));
            }
            let text = read_text()?;
            check(target)?;
            let after = unsafe { GetClipboardSequenceNumber() };
            if after != observed {
                observed = after;
                changed_at = Instant::now();
                continue;
            }
            if Instant::now() >= deadline {
                return Err(error("timeout", "读取复制结果超时，请重试或手动粘贴。"));
            }
            return Ok(CapturedText {
                request_id: request.id.clone(),
                text,
                elapsed_ms: request.started.elapsed().as_millis() as u64,
            });
        }
        thread::sleep(Duration::from_millis(8));
    }
    Err(error(
        "no_new_copy",
        "未检测到新的复制内容，请确认已选中文字；也可能是目标软件不支持或权限不同。",
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_whitespace_unicode_and_rejects_empty_malformed_or_oversized_text() {
        let units = "  cat\r\n😀  \0".encode_utf16().collect::<Vec<_>>();
        assert_eq!(decode(&units).unwrap(), "  cat\r\n😀  ");
        assert_eq!(decode(&[32, 0]).unwrap_err().code, "no_text");
        assert_eq!(decode(&[0xD800, 0]).unwrap_err().code, "invalid_text");
        assert_eq!(decode(&[65]).unwrap_err().code, "invalid_text");
        let mut long = vec![65; 2001];
        long.push(0);
        assert_eq!(decode(&long).unwrap_err().code, "too_long");
        let valid = "😀".repeat(2000) + "\0";
        assert!(decode(&valid.encode_utf16().collect::<Vec<_>>()).is_ok());
    }
}
