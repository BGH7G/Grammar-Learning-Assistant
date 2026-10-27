mod analysis;
#[cfg_attr(not(test), allow(dead_code))]
mod analyzer;
#[cfg(test)]
mod benchmark;
mod hotkey;
mod layered;
mod llm;
mod selection;
mod settings;
#[cfg(test)]
mod token_benchmark;
use analyzer::AnalysisMetrics;
use serde::Serialize;
use settings::{Settings, validate_settings};
use std::{
    fs,
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use tauri::Manager;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SettingsView {
    #[serde(flatten)]
    settings: Settings,
    has_key: bool,
}
struct State {
    client: reqwest::Client,
    lock: Mutex<()>,
    sessions: Mutex<layered::Store>,
    revision: AtomicU64,
    next_session: AtomicU64,
}
fn credential(s: &Settings) -> Result<keyring::Entry, String> {
    keyring::Entry::new("grammar-assistant", &s.endpoint)
        .map_err(|_| "无法访问 Windows 凭据管理器。".into())
}
fn path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|_| "无法定位配置目录。")?;
    fs::create_dir_all(&dir).map_err(|_| "无法创建配置目录。")?;
    Ok(dir.join("model.json"))
}
fn read_settings(app: &tauri::AppHandle) -> Result<Settings, String> {
    let p = path(app)?;
    if !p.exists() {
        return Ok(Settings::default());
    }
    serde_json::from_slice(&fs::read(p).map_err(|_| "读取配置失败。")?)
        .map_err(|_| "配置文件损坏，请重新保存模型配置。".into())
}
#[tauri::command]
fn get_settings(app: tauri::AppHandle, state: tauri::State<State>) -> Result<SettingsView, String> {
    let _guard = state.lock.lock().map_err(|_| "配置正在使用。")?;
    let s = read_settings(&app)?;
    let has_key = if s.endpoint.is_empty() {
        false
    } else {
        match credential(&s)?.get_password() {
            Ok(k) => !k.is_empty(),
            Err(keyring::Error::NoEntry) => false,
            Err(_) => return Err("读取系统凭据失败。".into()),
        }
    };
    Ok(SettingsView {
        settings: s,
        has_key,
    })
}
#[tauri::command]
fn save_settings(
    app: tauri::AppHandle,
    state: tauri::State<State>,
    mut settings: Settings,
    api_key: Option<String>,
    clear_key: bool,
) -> Result<(), String> {
    let _guard = state.lock.lock().map_err(|_| "配置正在使用。")?;
    settings.endpoint = settings.endpoint.trim().into();
    settings.model = settings.model.trim().into();
    validate_settings(&settings)?;
    let entry = credential(&settings)?;
    if clear_key {
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => (),
            Err(_) => return Err("删除密钥失败。".into()),
        }
    } else if let Some(key) = api_key.filter(|k| !k.trim().is_empty()) {
        entry
            .set_password(key.trim())
            .map_err(|_| "保存密钥失败。")?;
    }
    fs::write(
        path(&app)?,
        serde_json::to_vec_pretty(&settings).map_err(|_| "配置序列化失败。")?,
    )
    .map_err(|_| "保存配置失败。".to_owned())?;
    state.revision.fetch_add(1, Ordering::Relaxed);
    state
        .sessions
        .lock()
        .map_err(|_| "会话缓存不可用。")?
        .clear();
    Ok(())
}
fn configuration(
    app: &tauri::AppHandle,
    state: &State,
) -> Result<(Settings, String, String), String> {
    let _guard = state.lock.lock().map_err(|_| "配置正在使用。")?;
    let s = read_settings(app)?;
    validate_settings(&s)?;
    let key = match credential(&s)?.get_password() {
        Ok(k) => k,
        Err(keyring::Error::NoEntry) => String::new(),
        Err(_) => return Err("读取系统凭据失败。".into()),
    };
    let config_key = configuration_key(&s, state.revision.load(Ordering::Relaxed));
    Ok((s, key, config_key))
}
#[tauri::command]
async fn test_connection(
    app: tauri::AppHandle,
    state: tauri::State<'_, State>,
) -> Result<String, String> {
    let (s, key, _) = configuration(&app, &state)?;
    llm::completion(
        &state.client,
        &s,
        &key,
        serde_json::json!([{"role":"user","content":"Return JSON only: {\"ok\":true}"}]),
        512,
    )
    .await?;
    Ok("连接成功，模型已返回响应。".into())
}
fn configuration_key(s: &Settings, revision: u64) -> String {
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    layered::VERSION.hash(&mut hash);
    serde_json::to_string(s).unwrap_or_default().hash(&mut hash);
    format!("{revision}-{:x}", hash.finish())
}
fn ensure_configuration(
    app: &tauri::AppHandle,
    state: &State,
    expected: &str,
) -> Result<(), String> {
    let _guard = state.lock.lock().map_err(|_| "配置正在使用。")?;
    let current = read_settings(app)?;
    if configuration_key(&current, state.revision.load(Ordering::Relaxed)) != expected {
        return Err("模型配置已变化，请重新分析。".into());
    }
    Ok(())
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AnalysisResult {
    request_id: String,
    document: layered::Document,
    metrics: AnalysisMetrics,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ExpansionResult {
    request_id: String,
    session_id: String,
    layer: Option<layered::Layer>,
    error: Option<String>,
    metrics: AnalysisMetrics,
    cached: bool,
}

#[tauri::command]
async fn prepare_analysis(
    app: tauri::AppHandle,
    state: tauri::State<'_, State>,
    text: String,
    request_id: String,
    split_sentences: Option<bool>,
) -> Result<AnalysisResult, String> {
    let started = Instant::now();
    let (_, _, config_key) = configuration(&app, &state)?;
    let configuration_ms = started.elapsed().as_secs_f64() * 1000.0;
    let id = format!(
        "{}-{}",
        std::process::id(),
        state.next_session.fetch_add(1, Ordering::Relaxed)
    );
    let session = layered::Session::prepare(
        id,
        &text,
        config_key.clone(),
        split_sentences.unwrap_or(true),
    )?;
    ensure_configuration(&app, &state, &config_key)?;
    let document = state
        .sessions
        .lock()
        .map_err(|_| "会话缓存不可用。")?
        .prepare_or_reuse(session);
    // Local preparation only. Root requests use expand_node, sharing its cache,
    // in-flight guard, configuration checks and retry/usage accounting.
    Ok(AnalysisResult {
        request_id,
        document,
        metrics: AnalysisMetrics {
            total_ms: started.elapsed().as_secs_f64() * 1000.0,
            configuration_ms,
            preparation_ms: started.elapsed().as_secs_f64() * 1000.0 - configuration_ms,
            ..AnalysisMetrics::default()
        },
    })
}

#[tauri::command]
async fn analyze_sentence(
    app: tauri::AppHandle,
    state: tauri::State<'_, State>,
    text: String,
    request_id: String,
    split_sentences: Option<bool>,
) -> Result<AnalysisResult, String> {
    let started = Instant::now();
    let (s, key, config_key) = configuration(&app, &state)?;
    let configuration_ms = started.elapsed().as_secs_f64() * 1000.0;
    let id = format!(
        "{}-{}",
        std::process::id(),
        state.next_session.fetch_add(1, Ordering::Relaxed)
    );
    let mut session = layered::Session::prepare(
        id,
        &text,
        config_key.clone(),
        split_sentences.unwrap_or(true),
    )?;
    let mut metrics = layered::first_layers(&state.client, &s, &key, &mut session).await?;
    ensure_configuration(&app, &state, &config_key)?;
    let document = session.document();
    state
        .sessions
        .lock()
        .map_err(|_| "会话缓存不可用。")?
        .insert(session);
    metrics.configuration_ms = configuration_ms;
    metrics.total_ms = started.elapsed().as_secs_f64() * 1000.0;
    Ok(AnalysisResult {
        request_id,
        document,
        metrics,
    })
}

#[tauri::command]
async fn expand_node(
    app: tauri::AppHandle,
    state: tauri::State<'_, State>,
    session_id: String,
    node_id: String,
    request_id: String,
) -> Result<ExpansionResult, String> {
    let started = Instant::now();
    let (s, key, config_key) = configuration(&app, &state)?;
    let configuration_ms = started.elapsed().as_secs_f64() * 1000.0;
    let plan = state
        .sessions
        .lock()
        .map_err(|_| "会话缓存不可用。")?
        .plan(&session_id, &node_id, &config_key)?;
    match plan {
        layered::Plan::Cached(layer) => Ok(ExpansionResult {
            request_id,
            session_id,
            layer: Some(layer),
            error: None,
            cached: true,
            metrics: AnalysisMetrics {
                configuration_ms,
                total_ms: started.elapsed().as_secs_f64() * 1000.0,
                ..AnalysisMetrics::default()
            },
        }),
        layered::Plan::Fetch(session, node) => {
            let result = layered::query(&state.client, &s, &key, &session, &node).await;
            if let Err(error) = ensure_configuration(&app, &state, &config_key) {
                let _ = state
                    .sessions
                    .lock()
                    .map_err(|_| "会话缓存不可用。")?
                    .finish(&session_id, &node_id, &config_key, None);
                return Err(error);
            }
            match result {
                Ok((layer, mut metrics)) => {
                    state
                        .sessions
                        .lock()
                        .map_err(|_| "会话缓存不可用。")?
                        .finish(&session_id, &node_id, &config_key, Some(layer.clone()))?;
                    metrics.configuration_ms = configuration_ms;
                    metrics.total_ms = started.elapsed().as_secs_f64() * 1000.0;
                    Ok(ExpansionResult {
                        request_id,
                        session_id,
                        layer: Some(layer),
                        error: None,
                        metrics,
                        cached: false,
                    })
                }
                Err(mut error) => {
                    state
                        .sessions
                        .lock()
                        .map_err(|_| "会话缓存不可用。")?
                        .finish(&session_id, &node_id, &config_key, None)?;
                    error.metrics.configuration_ms = configuration_ms;
                    error.metrics.total_ms = started.elapsed().as_secs_f64() * 1000.0;
                    Ok(ExpansionResult {
                        request_id,
                        session_id,
                        layer: None,
                        error: Some(error.message),
                        metrics: error.metrics,
                        cached: false,
                    })
                }
            }
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let builder = builder.plugin(hotkey::plugin());
    builder
        .manage(hotkey::HotkeyState::default())
        .manage(selection::CaptureState::default())
        .setup(|app| {
            hotkey::setup(app.handle());
            Ok(())
        })
        .manage(State {
            client: reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(60))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .expect("HTTP client"),
            lock: Mutex::new(()),
            sessions: Mutex::new(layered::Store::default()),
            revision: AtomicU64::new(0),
            next_session: AtomicU64::new(1),
        })
        .invoke_handler(tauri::generate_handler![
            hotkey::get_hotkey_settings,
            hotkey::save_hotkey_settings,
            selection::get_selection_state,
            get_settings,
            save_settings,
            test_connection,
            prepare_analysis,
            analyze_sentence,
            expand_node
        ])
        .build(tauri::generate_context!())
        .expect("error building app")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                let state = app.state::<selection::CaptureState>();
                if state.request_exit() {
                    api.prevent_exit();
                }
            }
        });
}
#[cfg(test)]
mod live_tests {
    use super::*;

    // Opt-in only: uses the configured service and system credential; sends public
    // sample sentences, prints timings/counts only and never changes saved settings.
    #[test]
    #[ignore = "calls the user's configured model service"]
    fn configured_model_smoke() {
        let config = std::path::PathBuf::from(std::env::var("APPDATA").expect("APPDATA"))
            .join("com.example.grammarassistant")
            .join("model.json");
        let mut s: Settings =
            serde_json::from_slice(&fs::read(config).expect("Saved model settings")).unwrap();
        validate_settings(&s).unwrap();
        let key = credential(&s)
            .unwrap()
            .get_password()
            .expect("Saved system credential");
        s.thinking = false;
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(60))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();
        for (index, text) in [
            "The cat sleeps.",
            "The cat that I saw yesterday sat on the mat.",
        ]
        .iter()
        .enumerate()
        {
            let result = tauri::async_runtime::block_on(analyzer::analyze(&client, &s, &key, text))
                .unwrap_or_else(|e| panic!("Sample {index}: {e}"));
            assert!(result.analysis.nodes.iter().any(|n| n.role == "subject"));
            assert!(result.analysis.nodes.iter().any(|n| n.role == "predicate"));
            if index == 1 {
                assert!(
                    result
                        .analysis
                        .nodes
                        .iter()
                        .any(|n| n.kind == "clause" && n.parent_id.is_some())
                );
            }
            println!(
                "Sample {} metrics: {}",
                index + 1,
                serde_json::to_string(&result.metrics).unwrap()
            );
        }
    }
}
