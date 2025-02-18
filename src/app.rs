use leptos::task::spawn_local;
use leptos::{ev::SubmitEvent, prelude::*};
use serde::{Deserialize, Serialize};
use serde_json::json;
use wasm_bindgen::prelude::*;
use web_sys::console;
use futures::future::join_all;
use futures::future::FutureExt;
use gloo_timers::future::TimeoutFuture;
use leptos::html::ElementChild;
use leptos::attr::custom::CustomAttribute;

macro_rules! log {
    ($($t:tt)*) => {
        console::log_1(&format!($($t)*).into())
    }
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;
}

/// Wrapper for Tauri invoke with timeout and error handling
async fn invoke_with_timeout<T>(cmd: &str, args: JsValue, timeout_ms: u32) -> Result<T, String> 
where
    T: for<'a> Deserialize<'a>,
{
    let invoke_future = async {
        let result = invoke(cmd, args).await;
        serde_wasm_bindgen::from_value(result)
            .map_err(|e| format!("Failed to parse response: {}", e))
    };

    let timeout_future = TimeoutFuture::new(timeout_ms as u32)
        .map(|_| Err(format!("Request timed out after {}ms", timeout_ms)));

    futures::select! {
        result = invoke_future.fuse() => result,
        timeout = timeout_future.fuse() => timeout,
    }
}

#[derive(Serialize, Deserialize)]
struct GreetArgs<'a> {
    name: &'a str,
}

#[derive(Clone, Debug, Deserialize)]
struct SystemStatus {
    active_connections: usize,
    uptime: u64,
    memory_usage: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ModelConfig {
    name: String,
    provider: String,
} // End of ModelConfig definition

/// Configuration for a server connection and its selected model.
#[derive(Clone, Debug, Deserialize, Serialize)]
struct ServerConfig {
    id: String,
    name: String,
    url: String,
    provider: String,
    selected_model: String,
}

/// General configuration for the LLM application including the WebSocket URL.
/// Other fields (servers, available models, selected model) are added later.
#[derive(Clone, Debug, Deserialize, Serialize)]
struct LLMConfig {
    ws_url: String,
    servers: Vec<ServerConfig>,
    available_models: Vec<ModelConfig>,
    selected_model: String,
}

/// Server connection status
#[derive(Clone, Debug, PartialEq)]
enum ConnectionStatus {
    Unknown,
    Checking,
    Connected,
    Failed(String),
}

#[derive(Clone, Debug, Deserialize)]
struct LogEntry {
    level: String,
    message: String,
    timestamp: String,
    target: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChatCompletionRequest {
    #[serde(rename = "server_url")]
    server_url: String,
    model: String,
    message: String,
}

#[component]
fn ServerUrlInput(
    id: String,
    url: String,
    provider: String,
    name: String,
    selected_model: String,
    set_config: WriteSignal<LLMConfig>,
    server_statuses: ReadSignal<std::collections::HashMap<String, ConnectionStatus>>,
    set_server_statuses: WriteSignal<std::collections::HashMap<String, ConnectionStatus>>,
    check_connection: Box<dyn Fn(ServerConfig)>,
) -> impl IntoView {
    let id_for_input = id.clone();
    let id_for_update = id.clone();
    let id_for_checking = id.clone();
    let id_for_connected = id.clone();
    let id_for_failed = id.clone();
    let id_for_click = id.clone();
    let id_for_status = id.clone();
    let id_for_error = id.clone();

    let url_for_input = url.clone();
    let url_for_click = url.clone();
    let name_for_click = name.clone();
    let provider_for_click = provider.clone();
    let selected_model_for_click = selected_model.clone();

    view! {
        <div class="form-group">
            <label for=format!("server-url-{}", id_for_input)>"Server URL:"</label>
            <div class="server-url-container">
                <input
                    type="text"
                    id=format!("server-url-{}", id_for_input)
                    value=url_for_input
                    on:input=move |ev| {
                        let id = id_for_update.clone();
                        set_config.update(|c| {
                            if let Some(s) = c.servers.iter_mut().find(|s| s.id == id) {
                                s.url = event_target_value(&ev);
                            }
                        });
                        set_server_statuses.update(|s| { s.remove(&id); });
                    }
                />
                <button
                    type="button"
                    class="check-connection-btn"
                    class:checking=move || server_statuses.get().get(&id_for_checking).map_or(false, |s| matches!(s, ConnectionStatus::Checking))
                    class:connected=move || server_statuses.get().get(&id_for_connected).map_or(false, |s| matches!(s, ConnectionStatus::Connected))
                    class:failed=move || server_statuses.get().get(&id_for_failed).map_or(false, |s| matches!(s, ConnectionStatus::Failed(_)))
                    on:click=move |_| {
                        let server = ServerConfig {
                            id: id_for_click.clone(),
                            name: name_for_click.clone(),
                            url: url_for_click.clone(),
                            provider: provider_for_click.clone(),
                            selected_model: selected_model_for_click.clone(),
                        };
                        check_connection(server);
                    }
                >
                    {move || match server_statuses.get().get(&id_for_status).unwrap_or(&ConnectionStatus::Unknown) {
                        ConnectionStatus::Unknown => "Test Connection",
                        ConnectionStatus::Checking => "Checking...",
                        ConnectionStatus::Connected => "Connected ✓",
                        ConnectionStatus::Failed(_) => "Connection Failed ✗",
                    }}
                </button>
            </div>
            <div class="connection-error">
                {move || server_statuses.get()
                    .get(&id_for_error)
                    .and_then(|s| match s {
                        ConnectionStatus::Failed(err) => Some(err.clone()),
                        _ => None,
                    })
                    .unwrap_or_default()
                }
            </div>
        </div>
    }
}

#[component]
fn LogViewer() -> impl IntoView {
    let (show_logs, set_show_logs) = signal(false);
    let (logs, set_logs) = signal(Vec::<LogEntry>::new());
    let (filter_text, set_filter_text) = signal(String::new());
    let (selected_level, set_selected_level) = signal(String::from("all"));
    let (auto_scroll, set_auto_scroll) = signal(true);

    // Fetch logs periodically
    let _ = Effect::new(move |_| {
        if show_logs.get() {
            spawn_local(async move {
                loop {
                    let args = serde_wasm_bindgen::to_value(&()).unwrap_or(JsValue::NULL);
                    match invoke_with_timeout::<Vec<LogEntry>>("get_logs", args, 5000).await {
                        Ok(new_logs) => {
                            set_logs.set(new_logs);
                        }
                        Err(e) => log!("Failed to fetch logs: {}", e),
                    }
                    
                    // Wait for 1 second before next fetch
                    TimeoutFuture::new(1000).await;
                    
                    // Check if we should continue polling
                    if !show_logs.get_untracked() {
                        break;
                    }
                }
            });
        }
    });

    let filtered_logs = move || {
        let filter = filter_text.get().to_lowercase();
        let level = selected_level.get();
        logs.get()
            .into_iter()
            .filter(|log| {
                (level == "all" || log.level.to_lowercase() == level) &&
                (filter.is_empty() || 
                 log.message.to_lowercase().contains(&filter) ||
                 log.target.to_lowercase().contains(&filter))
            })
            .collect::<Vec<_>>()
    };

    let copy_logs = move |_| {
        let logs_text = filtered_logs()
            .into_iter()
            .map(|log| format!("{} [{}] [{}] {}", log.timestamp, log.level, log.target, log.message))
            .collect::<Vec<_>>()
            .join("\n");
        
        spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(&json!({ "text": logs_text })).unwrap();
            if let Err(e) = invoke_with_timeout::<()>("copy_to_clipboard", args, 1000).await {
                log!("Failed to copy logs: {}", e);
            }
        });
    };

    let clear_logs = move |_| {
        spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(&()).unwrap_or(JsValue::NULL);
            if let Err(e) = invoke_with_timeout::<()>("clear_logs", args, 1000).await {
                log!("Failed to clear logs: {}", e);
            } else {
                set_logs.set(Vec::new());
            }
        });
    };

    view! {
        <button
            class="logs-btn"
            class:active=move || show_logs.get()
            on:click=move |_| set_show_logs.update(|s| *s = !*s)
            title="View Logs"
        >
            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/>
            </svg>
        </button>

        {move || show_logs.get().then(|| view! {
            <div class="modal logs-modal">
                <div class="modal-content logs-content">
                    <div class="logs-header">
                        <h2>"System Logs"</h2>
                        <div class="logs-controls">
                            <div class="filter-group">
                                <input
                                    type="text"
                                    placeholder="Filter logs..."
                                    on:input=move |ev| set_filter_text.set(event_target_value(&ev))
                                />
                                <select
                                    on:change=move |ev| set_selected_level.set(event_target_value(&ev))
                                >
                                    <option value="all" selected=true>"All Levels"</option>
                                    <option value="error">"Error"</option>
                                    <option value="warn">"Warning"</option>
                                    <option value="info">"Info"</option>
                                    <option value="debug">"Debug"</option>
                                    <option value="trace">"Trace"</option>
                                </select>
                                <label class="auto-scroll-label">
                                    <input
                                        type="checkbox"
                                        checked=move || auto_scroll.get()
                                        on:change=move |ev| set_auto_scroll.set(event_target_checked(&ev))
                                    />
                                    "Auto-scroll"
                                </label>
                            </div>
                            <div class="logs-actions">
                                <button class="copy-btn" on:click=copy_logs title="Copy Logs">
                                    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                        <rect x="9" y="9" width="13" height="13" rx="2" ry="2"/>
                                        <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>
                                    </svg>
                                </button>
                                <button class="clear-btn" on:click=clear_logs title="Clear Logs">
                                    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                        <path d="M3 6h18"/>
                                        <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/>
                                    </svg>
                                </button>
                                <button class="close-btn" on:click=move |_| set_show_logs.set(false)>
                                    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                        <line x1="18" y1="6" x2="6" y2="18"/>
                                        <line x1="6" y1="6" x2="18" y2="18"/>
                                    </svg>
                                </button>
                            </div>
                        </div>
                    </div>
                    <div class="logs-container" id="logs-container">
                        {move || {
                            let logs = filtered_logs();
                            if auto_scroll.get() {
                                // Scroll to bottom after render
                                request_animation_frame(move || {
                                    if let Some(container) = document().get_element_by_id("logs-container") {
                                        container.set_scroll_top(container.scroll_height());
                                    }
                                });
                            }
                            logs.into_iter().map(|log| {
                                let LogEntry { level, timestamp, target, message } = log;
                                let level_lower = level.to_lowercase();
                                view! {
                                    <div class=format!("log-entry log-{}", level_lower)>
                                        <span class="log-timestamp">{format!("{}", timestamp)}</span>
                                        <span class=format!("log-level level-{}", level_lower)>
                                            {level}
                                        </span>
                                        <span class="log-target">{target}</span>
                                        <span class="log-message">{message}</span>
                                    </div>
                                }
                            }).collect_view()
                        }}
                    </div>
                </div>
            </div>
        })}
    }
}

#[component]
fn StatusBar() -> impl IntoView {
    let (status, set_status) = signal(SystemStatus {
        active_connections: 0,
        uptime: 0,
        memory_usage: 0.0,
    });
    let (show_settings, set_show_settings) = signal(false);
    let (config, set_config) = signal(LLMConfig {
        ws_url: String::from("ws://localhost:9001"),
        servers: vec![
            ServerConfig {
                id: "lmstudio-1".to_string(),
                name: "LM Studio".to_string(),
                url: "http://localhost:1234/v1".to_string(),
                provider: "LM Studio".to_string(),
                selected_model: String::new(),
            },
            ServerConfig {
                id: "ollama-1".to_string(),
                name: "Ollama".to_string(),
                url: "http://localhost:11434".to_string(),
                provider: "Ollama".to_string(),
                selected_model: String::new(),
            },
        ],
        available_models: vec![],
        selected_model: String::new(),
    });
    let (loading_models, set_loading_models) = signal(false);
    let (server_statuses, set_server_statuses) = signal(std::collections::HashMap::new());

    let toggle_settings = move |_| set_show_settings.update(|s| *s = !*s);
    
    let add_server = move |provider: &str| {
        set_config.update(|c| {
            let id = format!("{}-{}", provider.to_lowercase().replace(" ", "-"), c.servers.len() + 1);
            c.servers.push(ServerConfig {
                id,
                name: provider.to_string(),
                url: match provider {
                    "LM Studio" => "http://localhost:1234/v1".to_string(),
                    "Ollama" => "http://localhost:11434".to_string(),
                    _ => "http://localhost:8000".to_string(),
                },
                provider: provider.to_string(),
                selected_model: String::new(),
            });
        });
    };

    let remove_server = move |id: String| {
        set_config.update(|c| {
            c.servers.retain(|s| s.id != id);
        });
    };

    let fetch_models = move || {
        let set_loading_models = set_loading_models.clone();
        let set_config = set_config.clone();
        let config = config.clone();
        
        spawn_local(async move {
            log!("Starting to fetch models from all configured servers");
            set_loading_models.set(true);
            let mut models = Vec::new();
            let servers = config.get().servers.clone();

            // Create a vector of futures for parallel execution
            let futures: Vec<_> = servers.iter().map(|server| {
                let server = server.clone();
                async move {
                    log!("Fetching models from {} at {}", server.provider, server.url);
                    let base_url = server.url.trim_end_matches('/');
                    let full_url = if server.provider == "LM Studio" {
                        if base_url.ends_with("/v1") {
                            format!("{}/models", base_url)
                        } else {
                            format!("{}/v1/models", base_url)
                        }
                    } else {
                        format!("{}/api/tags", base_url)
                    };
                    
                    let args = serde_wasm_bindgen::to_value(&json!({
                        "url": full_url,
                        "timeout": 5000
                    })).map_err(|e| format!("Failed to serialize request: {}", e))?;
                    
                    let cmd = if server.provider == "LM Studio" { "fetch_models_lmstudio" } else { "fetch_models_ollama" };
                    
                    match invoke_with_timeout::<Result<Vec<String>, String>>(cmd, args, 10000).await {
                        Ok(Ok(models_result)) => {
                            log!("Successfully fetched {} models from {}", models_result.len(), server.provider);
                            Ok(models_result.into_iter().map(|name| ModelConfig {
                                name,
                                provider: server.provider.clone(),
                            }).collect::<Vec<_>>())
                        }
                        Ok(Err(err)) => {
                            log!("Server error from {}: {}", server.provider, err);
                            Err(format!("Server error: {}", err))
                        }
                        Err(err) => {
                            log!("Request failed for {}: {}", server.provider, err);
                            Err(format!("Request failed: {}", err))
                        }
                    }
                }
            }).collect();

            // Execute all futures in parallel with a timeout
            let results = join_all(futures).await;

            // Process results and handle errors
            let mut had_errors = false;
            for result in results {
                match result {
                    Ok(server_models) => models.extend(server_models),
                    Err(err) => {
                        log!("Error fetching models: {}", err);
                        had_errors = true;
                    }
                }
            }

            if models.is_empty() && had_errors {
                log!("Failed to fetch any models and encountered errors");
            } else {
                log!("Finished fetching models. Total models found: {}", models.len());
                set_config.update(|c| c.available_models = models);
            }
            set_loading_models.set(false);
        });
    };

    let save_settings = move |ev: SubmitEvent| {
        ev.prevent_default();
        spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(&config.get()).unwrap();
            let result = invoke("save_llm_config", args).await;
            if let Ok(_) = serde_wasm_bindgen::from_value::<()>(result) {
                set_show_settings.set(false);
            }
        });
    };

    let check_connection = move |server: ServerConfig| {
        let id = server.id.clone();
        let set_server_statuses = set_server_statuses.clone();
        
        spawn_local(async move {
            log!("Starting connection check for {} at {}", server.provider, server.url);
            set_server_statuses.update(|s| { s.insert(id.clone(), ConnectionStatus::Checking); });
            
            let base_url = server.url.trim_end_matches('/');
            let full_url = if server.provider == "LM Studio" {
                if base_url.ends_with("/v1") {
                    format!("{}/models", base_url)
                } else {
                    format!("{}/v1/models", base_url)
                }
            } else {
                format!("{}/api/tags", base_url)
            };
            
            let args = match serde_wasm_bindgen::to_value(&json!({
                "url": full_url,
                "timeout": 3000
            })) {
                Ok(args) => args,
                Err(e) => {
                    let error = format!("Failed to prepare request: {}", e);
                    log!("Connection check error: {}", error);
                    set_server_statuses.update(|s| { 
                        s.insert(id, ConnectionStatus::Failed(error)); 
                    });
                    return;
                }
            };
            
            let cmd = if server.provider == "LM Studio" { "fetch_models_lmstudio" } else { "fetch_models_ollama" };
            
            match invoke_with_timeout::<Result<Vec<String>, String>>(cmd, args, 5000).await {
                Ok(Ok(_)) => {
                    log!("Connection check successful for {} at {}", server.provider, server.url);
                    set_server_statuses.update(|s| { s.insert(id, ConnectionStatus::Connected); });
                }
                Ok(Err(err)) => {
                    let error = format!("Server error: {}", err);
                    log!("Connection check failed for {} at {}: {}", server.provider, server.url, error);
                    set_server_statuses.update(|s| { 
                        s.insert(id, ConnectionStatus::Failed(error)); 
                    });
                }
                Err(err) => {
                    let error = format!("Connection failed: {}", err);
                    log!("Connection check error for {} at {}: {}", server.provider, server.url, error);
                    set_server_statuses.update(|s| { 
                        s.insert(id, ConnectionStatus::Failed(error)); 
                    });
                }
            }
        });
    };

    // Initial model fetch when settings are opened
    let _ = Effect::new(move |_| {
        if show_settings.get() {
            fetch_models();
        }
    });

    // Initial model fetch when component is mounted
    let _ = Effect::new(move |_| {
        fetch_models();
    });

    // Update status every second
    let _ = Effect::new(move |_| {
        let status_update = async move {
            let set_status = set_status.clone();
            loop {
                let args = serde_wasm_bindgen::to_value(&()).map_err(|e| format!("Failed to serialize: {}", e)).unwrap_or_else(|e| {
                    log!("Error preparing status request: {}", e);
                    JsValue::NULL
                });

                match invoke_with_timeout::<SystemStatus>("get_system_status", args, 5000).await {
                    Ok(status) => set_status.set(status),
                    Err(e) => log!("Failed to update status: {}", e),
                }

                TimeoutFuture::new(1000).await;
            }
        };
        spawn_local(status_update);
    });

    view! {
        <div class="status-bar">
            <span class="status-item">"Connections: " {move || status.get().active_connections}</span>
            <span class="status-item">"Uptime: " {move || status.get().uptime} "s"</span>
            <span class="status-item">"Memory: " {move || format!("{:.1}%", status.get().memory_usage)}</span>
            <div class="status-item model-select-container">
                <select
                    class="model-select status-select"
                    on:change=move |ev| set_config.update(|c| c.selected_model = event_target_value(&ev))
                >
                    <option value="" selected=move || config.get().selected_model.is_empty()>
                        "Select model..."
                    </option>
                    {move || {
                        let servers = config.get().servers;
                        servers.into_iter().map(|server| {
                            let server_name = server.name.clone();
                            let server_provider = server.provider.clone();
                            let models = config.get().available_models
                                .iter()
                                .filter(|m| m.provider == server_provider)
                                .cloned()
                                .collect::<Vec<_>>();
                            
                            view! {
                                <optgroup label=server_name>
                                    {models.into_iter().map(|model| {
                                        let model_name = model.name;
                                        let model_name_for_value = model_name.clone();
                                        let model_name_for_selected = model_name.clone();
                                        let model_name_for_content = model_name.clone();
                                        let selected_model = config.get().selected_model.clone();
                                        view! {
                                            <option
                                                value=model_name_for_value
                                                selected=selected_model == model_name_for_selected
                                            >
                                                {model_name_for_content}
                                            </option>
                                        }
                                    }).collect_view()}
                                </optgroup>
                            }
                        }).collect_view()
                    }}
                </select>
                {move || loading_models.get().then(|| view! {
                    <span class="loading-spinner"></span>
                })}
            </div>
            <div class="status-actions">
                <LogViewer/>
                <button class="settings-btn" on:click=toggle_settings title="LLM Settings">
                    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                        <circle cx="12" cy="12" r="3"/>
                        <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"/>
                    </svg>
                </button>
            </div>
        </div>
        {move || show_settings.get().then(|| view! {
            <div class="modal">
                <div class="modal-content">
                    <h2>"LLM Settings"</h2>
                    <form on:submit=save_settings>
                        <div class="form-group">
                            <label for="ws-url">"WebSocket URL:"</label>
                            <input
                                type="text"
                                id="ws-url"
                                value=move || config.get().ws_url
                                on:input=move |ev| set_config.update(|c| c.ws_url = event_target_value(&ev))
                            />
                        </div>
                        <div class="servers-container">
                            <div class="servers-header">
                                <h3>"LLM Servers"</h3>
                                <div class="server-actions">
                                    <button
                                        type="button"
                                        class="add-server-btn"
                                        on:click=move |_| add_server("LM Studio")
                                    >
                                        "Add LM Studio"
                                    </button>
                                    <button
                                        type="button"
                                        class="add-server-btn"
                                        on:click=move |_| add_server("Ollama")
                                    >
                                        "Add Ollama"
                                    </button>
                                </div>
                            </div>
                            {move || {
                                let servers = config.get().servers;
                                servers.into_iter().map(|server| {
                                    let _name = server.name.clone();
                                    let id = server.id.clone();
                                    let provider = server.provider.clone();
                                    let selected_model = server.selected_model.clone();
                                    let url = server.url.clone();
                                    
                                    let id_for_div = id.clone();
                                    let id_for_url = id.clone();
                                    let id_for_model = id.clone();
                                    let id_for_model_input = id.clone();
                                    let provider_for_header = provider.clone();
                                    let provider_for_filter = provider.clone();
                                    
                                    view! {
                                        <div class="server-section" data-id=id_for_div>
                                            <div class="server-header">
                                                <h3>{format!("{} Server", provider_for_header)}</h3>
                                                <button
                                                    type="button"
                                                    class="remove-server-btn"
                                                    on:click=move |_| remove_server(id.clone())
                                                >
                                                    "Remove"
                                                </button>
                                            </div>
                                            <ServerUrlInput
                                                id=id_for_url.clone()
                                                url=url.clone()
                                                provider=provider.clone()
                                                name=provider_for_header.clone()
                                                selected_model=selected_model.clone()
                                                set_config=set_config
                                                server_statuses=server_statuses
                                                set_server_statuses=set_server_statuses
                                                check_connection=Box::new(check_connection.clone())
                                            />
                                            <div class="form-group">
                                                <label for=format!("server-model-{}", id_for_model)>"Model:"</label>
                                                <div class="select-wrapper">
                                                    <select
                                                        id=format!("server-model-{}", id_for_model_input)
                                                        class="model-select"
                                                        on:change=move |ev| {
                                                            let id = id_for_model.clone();
                                                            set_config.update(|c| {
                                                                if let Some(s) = c.servers.iter_mut().find(|s| s.id == id) {
                                                                    s.selected_model = event_target_value(&ev);
                                                                }
                                                            });
                                                        }
                                                    >
                                                        <option value="" selected=selected_model.is_empty()>
                                                            "Select a model..."
                                                        </option>
                                                        {move || {
                                                            let models = config.get().available_models
                                                                .iter()
                                                                .filter(|m| m.provider == provider_for_filter)
                                                                .cloned()
                                                                .collect::<Vec<_>>();
                                                            models.into_iter().map(|model| {
                                                                let model_name = model.name;
                                                                let model_name_for_value = model_name.clone();
                                                                let model_name_for_selected = model_name.clone();
                                                                let model_name_for_content = model_name.clone();
                                                                let selected_model = selected_model.clone();
                                                                view! {
                                                                    <option
                                                                        value=model_name_for_value
                                                                        selected=selected_model == model_name_for_selected
                                                                    >
                                                                        {model_name_for_content}
                                                                    </option>
                                                                }
                                                            }).collect_view()
                                                        }}
                                                    </select>
                                                </div>
                                            </div>
                                        </div>
                                    }
                                }).collect_view()
                            }}
                        </div>
                        <div class="form-group">
                            <label for="default-model">"Default Model:"</label>
                            <div class="select-wrapper">
                                <select
                                    id="default-model"
                                    class="model-select"
                                    on:change=move |ev| set_config.update(|c| c.selected_model = event_target_value(&ev))
                                >
                                    <option value="" selected=move || config.get().selected_model.is_empty()>
                                        "Select default model..."
                                    </option>
                                    {move || {
                                        let servers = config.get().servers;
                                        servers.into_iter().map(|server| {
                                            let server_name = server.name.clone();
                                            let server_provider = server.provider.clone();
                                            let models = config.get().available_models
                                                .iter()
                                                .filter(|m| m.provider == server_provider)
                                                .cloned()
                                                .collect::<Vec<_>>();
                                            
                                            view! {
                                                <optgroup label=server_name>
                                                    {models.into_iter().map(|model| {
                                                        let model_name = model.name;
                                                        let model_name_for_value = model_name.clone();
                                                        let model_name_for_selected = model_name.clone();
                                                        let model_name_for_content = model_name.clone();
                                                        let selected_model = config.get().selected_model.clone();
                                                        view! {
                                                            <option
                                                                value=model_name_for_value
                                                                selected=selected_model == model_name_for_selected
                                                            >
                                                                {model_name_for_content}
                                                            </option>
                                                        }
                                                    }).collect_view()}
                                                </optgroup>
                                            }
                                        }).collect_view()
                                    }}
                                </select>
                                {move || loading_models.get().then(|| view! {
                                    <span class="loading-spinner"></span>
                                })}
                            </div>
                        </div>
                        <div class="form-actions">
                            <button type="button" on:click=toggle_settings>"Cancel"</button>
                            <button type="submit">"Save Configuration"</button>
                        </div>
                    </form>
                </div>
            </div>
        })}
    }
}

#[component]
pub fn App() -> impl IntoView {
    let (name, set_name) = signal(String::new());
    let (greet_msg, set_greet_msg) = signal(String::new());

    let update_name = move |ev| {
        let v = event_target_value(&ev);
        set_name.set(v);
    };

    let greet = move |ev: SubmitEvent| {
        ev.prevent_default();
        spawn_local(async move {
            let name = name.get_untracked();
            if name.is_empty() {
                return;
            }

            let args = serde_wasm_bindgen::to_value(&GreetArgs { name: &name }).unwrap();
            // Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
            let new_msg = invoke("greet", args).await.as_string().unwrap();
            set_greet_msg.set(new_msg);
        });
    };

    view! {
        <main class="container">
            <h1>"Welcome to Tauri + Leptos"</h1>

            <div class="row">
                <a href="https://tauri.app" target="_blank">
                    <img src="public/tauri.svg" class="logo tauri" alt="Tauri logo"/>
                </a>
                <a href="https://docs.rs/leptos/" target="_blank">
                    <img src="public/leptos.svg" class="logo leptos" alt="Leptos logo"/>
                </a>
            </div>
            <p>"Click on the Tauri and Leptos logos to learn more."</p>

            <form class="row" on:submit=greet>
                <input
                    id="greet-input"
                    placeholder="Enter a name..."
                    on:input=update_name
                />
                <button type="submit">"Greet"</button>
            </form>
            <p>{ move || greet_msg.get() }</p>
            <StatusBar/>
        </main>
    }
}
