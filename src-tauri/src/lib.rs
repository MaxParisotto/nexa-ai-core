// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use serde::{Deserialize, Serialize};
use tauri::State;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use reqwest::Client;
use std::collections::VecDeque;
use std::time::Duration;
use tauri_plugin_log::Target;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LogEntry {
    level: String,
    message: String,
    timestamp: String,
    target: String,
}

struct LogState {
    entries: VecDeque<LogEntry>,
}

impl LogState {
    fn new() -> Self {
        Self {
            entries: VecDeque::with_capacity(1000), // Keep last 1000 logs
        }
    }

    fn add_entry(&mut self, level: &str, message: &str, target: &str) -> Result<(), String> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?;
        
        let entry = LogEntry {
            level: level.to_string(),
            message: message.to_string(),
            timestamp: chrono::DateTime::from_timestamp(timestamp.as_secs() as i64, 0)
                .ok_or_else(|| "Failed to create timestamp".to_string())?
                .format("%Y-%m-%d %H:%M:%S")
                .to_string(),
            target: target.to_string(),
        };

        if self.entries.len() >= 1000 {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
        Ok(())
    }
}

#[tauri::command]
async fn get_logs(log_state: State<'_, Mutex<LogState>>) -> Result<Vec<LogEntry>, String> {
    let entries = {
        let state = log_state.lock().map_err(|e| e.to_string())?;
        state.entries.iter().cloned().collect()
    };
    Ok(entries)
}

#[tauri::command]
async fn clear_logs(log_state: State<'_, Mutex<LogState>>) -> Result<(), String> {
    let mut state = log_state.lock().map_err(|e| e.to_string())?;
    state.entries.clear();
    Ok(())
}

fn add_log_entry(log_state: &State<'_, Mutex<LogState>>, level: &str, message: &str, target: &str) -> Result<(), String> {
    let mut state = log_state.lock().map_err(|e| e.to_string())?;
    state.add_entry(level, message, target)
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct OpenAIModel {
    id: String,
    object: String,
    owned_by: String,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct OpenAIModelList {
    data: Vec<OpenAIModel>,
    object: String,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct OllamaModel {
    name: String,
    model: String,
    modified_at: String,
    size: u64,
    digest: String,
}

#[derive(Deserialize)]
struct OllamaResponse {
    models: Vec<OllamaModel>,
}

#[tauri::command]
async fn fetch_models_lmstudio(url: String, timeout: u64, log_state: State<'_, Mutex<LogState>>) -> Result<Result<Vec<String>, String>, String> {
    // Ensure we have the correct models endpoint
    let base_url = url.trim_end_matches('/');
    let models_url = if base_url.ends_with("/v1") {
        format!("{}/models", base_url)
    } else if !base_url.contains("/v1/") {
        format!("{}/v1/models", base_url)
    } else {
        base_url.to_string()
    };
    
    add_log_entry(&log_state, "info", &format!("Checking LM Studio server at {}", models_url), "server_check")?;

    let client = Client::builder()
        .timeout(Duration::from_millis(timeout))
        .build()
        .map_err(|e| {
            let err = e.to_string();
            let _ = add_log_entry(&log_state, "error", &format!("Failed to create HTTP client: {}", err), "server_check");
            err
        })?;

    match client.get(&models_url).send().await {
        Ok(response) => {
            if response.status().is_success() {
                match response.json::<OpenAIModelList>().await {
                    Ok(model_list) => {
                        let models: Vec<String> = model_list.data.into_iter().map(|m| m.id).collect();
                        let _ = add_log_entry(
                            &log_state,
                            "info",
                            &format!("Successfully fetched {} models from LM Studio", models.len()),
                            "server_check"
                        );
                        Ok(Ok(models))
                    },
                    Err(e) => {
                        let err = format!("Failed to parse response: {}", e);
                        let _ = add_log_entry(&log_state, "error", &err, "server_check");
                        Ok(Err(err))
                    }
                }
            } else {
                let err = format!("Server returned error: {}", response.status());
                let _ = add_log_entry(&log_state, "error", &err, "server_check");
                Ok(Err(err))
            }
        }
        Err(e) => {
            let err = format!("Request failed: {}", e);
            let _ = add_log_entry(&log_state, "error", &err, "server_check");
            Ok(Err(err))
        }
    }
}

#[tauri::command]
async fn fetch_models_ollama(url: String, timeout: u64, log_state: State<'_, Mutex<LogState>>) -> Result<Result<Vec<String>, String>, String> {
    // Ensure we have the correct tags endpoint
    let base_url = url.trim_end_matches('/');
    let tags_url = format!("{}/api/tags", base_url);
    
    add_log_entry(&log_state, "info", &format!("Checking Ollama server at {}", tags_url), "server_check")?;

    let client = Client::builder()
        .timeout(Duration::from_millis(timeout))
        .build()
        .map_err(|e| {
            let err = e.to_string();
            let _ = add_log_entry(&log_state, "error", &format!("Failed to create HTTP client: {}", err), "server_check");
            err
        })?;

    match client.get(&tags_url).send().await {
        Ok(response) => {
            if response.status().is_success() {
                match response.json::<OllamaResponse>().await {
                    Ok(model_list) => {
                        let models: Vec<String> = model_list.models.into_iter()
                            .map(|m| m.name)
                            .collect();
                        let _ = add_log_entry(
                            &log_state,
                            "info",
                            &format!("Successfully fetched {} models from Ollama", models.len()),
                            "server_check"
                        );
                        Ok(Ok(models))
                    },
                    Err(e) => {
                        let err = format!("Failed to parse response: {}", e);
                        let _ = add_log_entry(&log_state, "error", &err, "server_check");
                        Ok(Err(err))
                    }
                }
            } else {
                let err = format!("Server returned error: {}", response.status());
                let _ = add_log_entry(&log_state, "error", &err, "server_check");
                Ok(Err(err))
            }
        }
        Err(e) => {
            let err = format!("Request failed: {}", e);
            let _ = add_log_entry(&log_state, "error", &err, "server_check");
            Ok(Err(err))
        }
    }
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let log_state = LogState::new();

    tauri::Builder::default()
        .manage(Mutex::new(log_state))
        .plugin(tauri_plugin_window_state::Builder::new().build())
        .plugin(tauri_plugin_websocket::init())
        .plugin(tauri_plugin_upload::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_persisted_scope::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_log::Builder::new()
            .targets([
                Target::new(tauri_plugin_log::TargetKind::Stdout),
                Target::new(tauri_plugin_log::TargetKind::Webview),
                Target::new(tauri_plugin_log::TargetKind::LogDir { file_name: None })
            ])
            .format(|out, message, record| {
                out.finish(format_args!(
                    "{} [{}] [{}] {}",
                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                    record.level(),
                    record.target(),
                    message
                ))
            })
            .level(log::LevelFilter::Debug)
            .build())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_cli::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            get_logs,
            clear_logs,
            fetch_models_lmstudio,
            fetch_models_ollama
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

