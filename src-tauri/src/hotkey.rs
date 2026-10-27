//! Persist and register the one clipboard-selection shortcut in Rust.
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};
use tauri::Manager;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

pub(crate) const DEFAULT_SHORTCUT: &str = "Ctrl+Alt+E";
#[derive(Default)]
pub(crate) struct HotkeyState {
    runtime: Mutex<Runtime>,
    active: AtomicU64,
    pressed: AtomicBool,
}
#[derive(Default)]
struct Runtime {
    registered: Option<Shortcut>,
    error: Option<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HotkeyView {
    #[serde(flatten)]
    settings: HotkeySettings,
    registered: bool,
    registration_error: Option<String>,
}
fn view(settings: HotkeySettings, runtime: &Runtime) -> HotkeyView {
    let registered = to_shortcut(&settings.selection_translate).ok() == runtime.registered;
    HotkeyView {
        settings,
        registered,
        registration_error: runtime.error.clone(),
    }
}
fn to_shortcut(value: &str) -> Result<Shortcut, String> {
    value
        .replace("Win+", "Super+")
        .parse()
        .map_err(|_| "快捷键格式无效，请重新设置。".into())
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct HotkeySettings {
    pub selection_translate: String,
}
impl Default for HotkeySettings {
    fn default() -> Self {
        Self {
            selection_translate: DEFAULT_SHORTCUT.into(),
        }
    }
}
pub(crate) fn plugin<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri_plugin_global_shortcut::Builder::new().build()
}
fn normalize(value: &str) -> Result<String, String> {
    let value = value.trim().replace("Win+", "Super+");
    let shortcut: Shortcut = value
        .parse()
        .map_err(|_| "快捷键格式无效，请重新按下组合键。")?;
    if shortcut.mods == Modifiers::CONTROL && shortcut.key == Code::KeyC {
        return Err("Ctrl+C 用于系统复制，不能作为划词翻译热键。".into());
    }
    if !shortcut
        .mods
        .intersects(Modifiers::CONTROL | Modifiers::ALT | Modifiers::SUPER)
    {
        return Err("请使用包含 Ctrl、Alt 或 Win 的组合键。".into());
    }
    let mut parts = Vec::new();
    for (modifier, label) in [
        (Modifiers::CONTROL, "Ctrl"),
        (Modifiers::ALT, "Alt"),
        (Modifiers::SHIFT, "Shift"),
        (Modifiers::SUPER, "Win"),
    ] {
        if shortcut.mods.contains(modifier) {
            parts.push(label.to_string());
        }
    }
    let key = shortcut.key.to_string();
    parts.push(
        key.strip_prefix("Key")
            .or_else(|| key.strip_prefix("Digit"))
            .unwrap_or(&key)
            .to_string(),
    );
    Ok(parts.join("+"))
}
fn path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map(|dir| dir.join("hotkeys.json"))
        .map_err(|_| "无法定位热键配置目录。".into())
}
fn read(path: &Path) -> Result<HotkeySettings, String> {
    match fs::read(path) {
        Ok(bytes) => {
            let mut settings: HotkeySettings =
                serde_json::from_slice(&bytes).map_err(|_| "热键配置损坏，请重新设置并保存。")?;
            settings.selection_translate = normalize(&settings.selection_translate)?;
            Ok(settings)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(HotkeySettings::default()),
        Err(_) => Err("读取热键配置失败。".into()),
    }
}
fn write(path: &Path, settings: &HotkeySettings) -> Result<(), String> {
    let parent = path.parent().ok_or("热键配置路径无效。")?;
    fs::create_dir_all(parent).map_err(|_| "无法创建热键配置目录。")?;
    let bytes = serde_json::to_vec_pretty(settings).map_err(|_| "热键配置序列化失败。")?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, bytes).map_err(|_| "写入热键配置失败。")?;
    if fs::rename(&temporary, path).is_err() {
        let _ = fs::remove_file(&temporary);
        return Err("保存热键配置失败，原配置未更改。".into());
    }
    Ok(())
}
fn register(app: &tauri::AppHandle, shortcut: Shortcut) -> Result<(), String> {
    app.global_shortcut()
        .on_shortcut(shortcut, |app, _, event| {
            let state = app.state::<HotkeyState>();
            if state.active.load(Ordering::Acquire) != u64::from(event.id) + 1 {
                return;
            }
            if event.state == ShortcutState::Released {
                state.pressed.store(false, Ordering::Release);
                return;
            }
            if state.pressed.swap(true, Ordering::AcqRel) {
                return;
            }
            if let Some(target) = crate::selection::target_window() {
                app.state::<crate::selection::CaptureState>()
                    .submit(app, target);
            }
        })
        .map_err(|_| "快捷键注册失败，组合键可能已被占用，请换一个组合键。".into())
}
fn apply(
    app: &tauri::AppHandle,
    settings: HotkeySettings,
    persist: bool,
) -> Result<HotkeyView, String> {
    // Resolve persistence before changing any operating-system registrations.
    let destination = if persist { Some(path(app)?) } else { None };
    let state = app.state::<HotkeyState>();
    let mut runtime = state.runtime.lock().map_err(|_| "热键设置正在使用。")?;
    if app.state::<crate::selection::CaptureState>().busy() {
        return Err("正在取词，请稍后再修改快捷键。".into());
    }
    let next = to_shortcut(&settings.selection_translate)?;
    let previous = runtime.registered;
    if previous != Some(next) {
        register(app, next)?;
        if let Some(old) = previous
            && app.global_shortcut().unregister(old).is_err()
        {
            let _ = app.global_shortcut().unregister(next);
            return Err("原快捷键注销失败，已保留原设置。".into());
        }
    }
    if let Some(destination) = destination
        && let Err(message) = write(&destination, &settings)
    {
        if previous != Some(next) {
            let _ = app.global_shortcut().unregister(next);
            runtime.registered = previous.filter(|old| register(app, *old).is_ok());
            state.active.store(
                runtime
                    .registered
                    .map(|s| u64::from(s.id()) + 1)
                    .unwrap_or(0),
                Ordering::Release,
            );
            if previous.is_some() && runtime.registered.is_none() {
                runtime.error = Some("保存失败且原热键恢复失败，请重新保存热键设置。".into());
                app.state::<crate::selection::CaptureState>()
                    .set_hotkey_error(app, runtime.error.clone());
            }
        }
        return Err(message);
    }
    runtime.registered = Some(next);
    runtime.error = None;
    state.pressed.store(false, Ordering::Release);
    state
        .active
        .store(u64::from(next.id()) + 1, Ordering::Release);
    app.state::<crate::selection::CaptureState>()
        .set_hotkey_error(app, None);
    Ok(view(settings, &runtime))
}
pub(crate) fn setup(app: &tauri::AppHandle) {
    let result = app
        .state::<crate::selection::CaptureState>()
        .start(app.clone())
        .and_then(|_| path(app).and_then(|p| read(&p)))
        .and_then(|settings| apply(app, settings, false));
    if let Err(message) = result {
        if let Ok(mut runtime) = app.state::<HotkeyState>().runtime.lock() {
            runtime.error = Some(message.clone());
        }
        app.state::<crate::selection::CaptureState>()
            .set_hotkey_error(app, Some(message));
    }
}
#[tauri::command]
pub(crate) fn get_hotkey_settings(
    app: tauri::AppHandle,
    state: tauri::State<HotkeyState>,
) -> Result<HotkeyView, String> {
    let runtime = state.runtime.lock().map_err(|_| "热键设置正在使用。")?;
    Ok(view(read(&path(&app)?)?, &runtime))
}
#[tauri::command]
pub(crate) fn save_hotkey_settings(
    app: tauri::AppHandle,
    shortcut: String,
) -> Result<HotkeyView, String> {
    let settings = HotkeySettings {
        selection_translate: normalize(&shortcut)?,
    };
    apply(&app, settings, true)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normalizes_valid_chords_and_rejects_incomplete_or_unmodified_keys() {
        assert_eq!(normalize("control+shift+KeyK").unwrap(), "Ctrl+Shift+K");
        assert_eq!(normalize("Win+Alt+Digit1").unwrap(), "Alt+Win+1");
        assert_eq!(normalize("Ctrl+F12").unwrap(), "Ctrl+F12");
        for invalid in ["E", "Shift+A", "Ctrl+Alt", "Ctrl+E+K", "Ctrl+C", ""] {
            assert!(normalize(invalid).is_err(), "{invalid}");
        }
    }
    #[test]
    fn preferences_survive_reload_and_replacement_without_model_settings() {
        let dir = std::env::temp_dir().join(format!(
            "grammar-hotkey-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = dir.join("hotkeys.json");
        assert_eq!(read(&path).unwrap(), HotkeySettings::default());
        let first = HotkeySettings {
            selection_translate: "Ctrl+Shift+K".into(),
        };
        write(&path, &first).unwrap();
        assert_eq!(read(&path).unwrap(), first);
        let second = HotkeySettings::default();
        write(&path, &second).unwrap();
        assert_eq!(read(&path).unwrap(), second);
        assert!(!dir.join("model.json").exists());
        fs::remove_file(path).unwrap();
        fs::remove_dir(dir).unwrap();
    }
}
