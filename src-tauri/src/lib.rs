// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use serde::{Deserialize, Serialize};
use tauri::{State, Manager};
use std::sync::{Mutex, Arc};
use std::time::{SystemTime, UNIX_EPOCH, Instant};
use reqwest::Client;
use std::collections::VecDeque;
use std::time::Duration;
use tauri_plugin_log::Target;
use serde_json::{self, json};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{accept_async, tungstenite::Message, WebSocketStream};
use futures_util::{StreamExt, SinkExt};
use log;
use serde_json::Value;
use tokio::sync::mpsc;
use sysinfo::System;

// Define WebSocket types
type WebSocket = WebSocketStream<TcpStream>;
type WsSender = mpsc::Sender<Message>;
static WS_SENDER: tokio::sync::Mutex<Option<WsSender>> = tokio::sync::Mutex::const_new(None);

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LogEntry {
    pub level: String,
    pub message: String, 
    pub timestamp: String,
    pub target: String,
}

#[derive(Debug)]
pub struct LogState {
    pub entries: VecDeque<LogEntry>,
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

#[derive(Clone)]
struct SystemState {
    start_time: Instant,
    active_connections: Arc<Mutex<usize>>,
}

impl SystemState {
    fn new() -> Self {
        Self {
            start_time: Instant::now(),
            active_connections: Arc::new(Mutex::new(0)),
        }
    }

    #[allow(dead_code)]
    fn increment_connections(&self) {
        if let Ok(mut count) = self.active_connections.lock() {
            *count += 1;
        }
    }

    #[allow(dead_code)]
    fn decrement_connections(&self) {
        if let Ok(mut count) = self.active_connections.lock() {
            if *count > 0 {
                *count -= 1;
            }
        }
    }

    fn get_connections(&self) -> usize {
        self.active_connections.lock().map(|count| *count).unwrap_or(0)
    }

    fn get_uptime(&self) -> u64 {
        self.start_time.elapsed().as_secs()
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

#[derive(Debug, Serialize, Deserialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    stream: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChatResponse {
    message: Option<ChatMessage>,
    choices: Option<Vec<Choice>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Choice {
    message: ChatMessage,
}

#[derive(Debug, Serialize)]
struct ProcessInfo {
    name: String,
    pid: u32,
    cpu_usage: f32,
    memory_usage: u64,
    status: String,
}

#[derive(Debug, Serialize)]
struct SystemStatus {
    active_connections: usize,
    uptime: u64,
    memory_usage: f64,
    cpu_usage: f32,
    processes: Vec<ProcessInfo>,
}

#[tauri::command]
async fn get_system_status(system_state: State<'_, SystemState>) -> Result<SystemStatus, String> {
    let mut sys = System::new();
    
    // Refresh only what we need
    sys.refresh_memory();
    sys.refresh_cpu_all();
    sys.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::Some(&[sysinfo::Pid::from(std::process::id() as usize)]),
        true,
        sysinfo::ProcessRefreshKind::everything()
    );
    
    // Get memory usage with error handling
    let total_memory = sys.total_memory() as f64;
    let used_memory = (total_memory - sys.available_memory() as f64) as f64;
    let memory_usage = if total_memory > 0.0 {
        (used_memory / total_memory) * 100.0
    } else {
        0.0
    };
    
    // Get CPU usage with error handling
    let cpu_usage = if sys.cpus().is_empty() {
        0.0
    } else {
        sys.cpus().iter().map(|cpu| cpu.cpu_usage()).sum::<f32>() / sys.cpus().len() as f32
    };
    
    // Get current process and its children with improved error handling
    let current_pid = std::process::id() as usize;
    let mut processes = Vec::new();
    
    if let Some(process) = sys.process(sysinfo::Pid::from(current_pid)) {
        let process_info = ProcessInfo {
            name: process.name().to_string_lossy().into_owned(),
            pid: current_pid as u32,
            cpu_usage: process.cpu_usage(),
            memory_usage: process.memory(),
            status: format!("{:?}", process.status()),
        };
        processes.push(process_info);
        
        // Get child processes with improved error handling
        for (pid, proc) in sys.processes() {
            if let Some(parent_pid) = proc.parent() {
                if parent_pid == sysinfo::Pid::from(current_pid) {
                    let child_info = ProcessInfo {
                        name: proc.name().to_string_lossy().into_owned(),
                        pid: pid.as_u32(),
                        cpu_usage: proc.cpu_usage(),
                        memory_usage: proc.memory(),
                        status: format!("{:?}", proc.status()),
                    };
                    processes.push(child_info);
                }
            }
        }
    }

    Ok(SystemStatus {
        active_connections: system_state.get_connections(),
        uptime: system_state.get_uptime(),
        memory_usage,
        cpu_usage,
        processes,
    })
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
    let base_url = url.trim_end_matches('/').trim_end_matches("/api/tags");
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

async fn ollama_chat(server_url: &str, model: &str, message: &str, temperature: f32) -> Result<String, String> {
    let client = Client::new();
    let url = format!("{}/api/chat", server_url.trim_end_matches('/'));
    
    let response = client.post(&url)
        .json(&json!({
            "model": model,
            "messages": [{
                "role": "user",
                "content": message
            }],
            "temperature": temperature,
            "stream": false
        }))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if response.status().is_success() {
        let result: Value = response.json().await.map_err(|e| e.to_string())?;
        Ok(result["message"]["content"].as_str()
            .ok_or_else(|| "Invalid response format".to_string())?
            .to_string())
    } else {
        Err(format!("Request failed: {}", response.status()))
    }
}

async fn lmstudio_chat(server_url: &str, model: &str, message: &str, temperature: f32) -> Result<String, String> {
    let client = Client::new();
    let url = format!("{}/chat/completions", server_url.trim_end_matches('/'));
    
    let response = client.post(&url)
        .json(&json!({
            "model": model,
            "messages": [{
                "role": "user",
                "content": message
            }],
            "temperature": temperature,
            "stream": false
        }))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if response.status().is_success() {
        let result: Value = response.json().await.map_err(|e| e.to_string())?;
        Ok(result["choices"][0]["message"]["content"].as_str()
            .ok_or_else(|| "Invalid response format".to_string())?
            .to_string())
    } else {
        Err(format!("Request failed: {}", response.status()))
    }
}

#[tauri::command]
async fn chat_completion(
    server_url: String,
    model: String,
    message: String,
    temperature: f32,
) -> Result<String, String> {
    let response = match server_url.contains("11434") {
        true => ollama_chat(&server_url, &model, &message, temperature).await,
        false => lmstudio_chat(&server_url, &model, &message, temperature).await,
    };

    // Send the response through WebSocket for real-time updates
    if let Ok(response_text) = &response {
        if let Some(tx) = WS_SENDER.lock().await.as_ref() {
            let update = json!({
                "type": "chat_response",
                "data": {
                    "model": model,
                    "message": message,
                    "response": response_text,
                    "timestamp": chrono::Local::now().to_rfc3339()
                }
            });
            
            let _ = tx.send(Message::Text(update.to_string().into())).await;
        }
    }

    response
}

// WebSocket handler for real-time updates
pub async fn handle_ws_connection(ws: WebSocket) {
    let (mut tx, mut rx) = ws.split();
    let (sender, mut receiver) = mpsc::channel::<Message>(32);
    
    // Store sender for broadcasting updates
    *WS_SENDER.lock().await = Some(sender);

    // Forward received messages to all connected clients
    let forward_task = tokio::spawn(async move {
        while let Some(msg) = receiver.recv().await {
            if let Err(e) = tx.send(msg).await {
                log::error!("Failed to forward message: {}", e);
                break;
            }
        }
    });

    // Handle incoming messages
    let receive_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = rx.next().await {
            match msg {
                Message::Text(text) => {
                    log::info!("Received message: {}", text);
                    // Handle incoming message routing
                    if let Ok(update) = serde_json::from_str::<Value>(&text) {
                        if let Some(msg_type) = update.get("type").and_then(|t| t.as_str()) {
                            match msg_type {
                                "node_output" => {
                                    // Handle node output routing
                                    if let Some(tx) = WS_SENDER.lock().await.as_ref() {
                                        let _ = tx.send(Message::Text(text.into())).await;
                                    }
                                }
                                _ => log::warn!("Unknown message type: {}", msg_type),
                            }
                        }
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    // Wait for tasks to complete
    tokio::select! {
        _ = forward_task => {},
        _ = receive_task => {},
    }
}

#[tauri::command]
async fn register_connection(system_state: State<'_, SystemState>) -> Result<(), String> {
    system_state.increment_connections();
    Ok(())
}

#[tauri::command]
async fn unregister_connection(system_state: State<'_, SystemState>) -> Result<(), String> {
    system_state.decrement_connections();
    Ok(())
}

pub async fn start_websocket_server() -> Result<(), String> {
    let port = 9001;
    let addr = format!("127.0.0.1:{}", port);

    let listener = TcpListener::bind(&addr)
        .await
        .map_err(|e| format!("Failed to bind to address: {}", e))?;

    log::info!("WebSocket server listening on {}", addr);

    tokio::spawn(async move {
        while let Ok((stream, addr)) = listener.accept().await {
            log::info!("New WebSocket connection from: {}", addr);
            
            tokio::spawn(async move {
                match handle_connection(stream).await {
                    Ok(_) => log::info!("WebSocket connection closed gracefully: {}", addr),
                    Err(e) => log::error!("WebSocket connection error: {}", e),
                }
            });
        }
    });

    Ok(())
}

async fn handle_connection(stream: TcpStream) -> Result<(), String> {
    let ws_stream = accept_async(stream)
        .await
        .map_err(|e| format!("Failed to accept WebSocket connection: {}", e))?;

    let (mut tx, mut rx) = ws_stream.split();

    // Handle incoming messages only
    while let Some(msg) = rx.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                log::debug!("Received text message: {}", text);
                // Echo back with a timestamp
                let response = format!("Received at {}: {}", 
                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                    text
                );
                if let Err(e) = tx.send(Message::Text(response.into())).await {
                    log::error!("Failed to send response: {}", e);
                    break;
                }
            }
            Ok(Message::Close(_)) => {
                log::info!("Client initiated close");
                break;
            }
            Ok(_) => {
                // Ignore other message types
                continue;
            }
            Err(e) => {
                return Err(format!("WebSocket error: {}", e));
            }
        }
    }

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let log_state = LogState::new();
    let system_state = SystemState::new();

    tauri::Builder::default()
        .manage(Mutex::new(log_state))
        .manage(system_state)
        .plugin(tauri_plugin_window_state::Builder::new().build())
        .plugin(tauri_plugin_websocket::init())
        .plugin(tauri_plugin_upload::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_persisted_scope::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_log::Builder::new()
            .targets([
                Target::new(tauri_plugin_log::TargetKind::Stdout),
                Target::new(tauri_plugin_log::TargetKind::Webview),
                Target::new(tauri_plugin_log::TargetKind::LogDir { 
                    file_name: Some("nexa-ai.log".into()) 
                })
            ])
            .format(|out, message, record| {
                out.finish(format_args!(
                    "{} [{}] [{}] {}",
                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
                    record.level(),
                    record.target(),
                    message
                ))
            })
            .level(log::LevelFilter::Debug)
            .build())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_cli::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let log_state = app.state::<Mutex<LogState>>();
            let _system_state = app.state::<SystemState>();
            
            // Initialize with startup log entries
            if let Ok(mut state) = log_state.lock() {
                let _ = state.add_entry(
                    "info",
                    "Application started",
                    "system"
                );
                let _ = state.add_entry(
                    "debug",
                    "Registering Tauri commands: greet, get_logs, clear_logs, fetch_models_lmstudio, fetch_models_ollama, chat_completion, get_system_status",
                    "system"
                );
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            get_logs,
            clear_logs,
            fetch_models_lmstudio,
            fetch_models_ollama,
            chat_completion,
            get_system_status,
            register_connection,
            unregister_connection
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

