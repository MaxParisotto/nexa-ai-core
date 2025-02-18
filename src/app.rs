use leptos::task::spawn_local;
use leptos::prelude::*;
use leptos::ev::SubmitEvent;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsValue;
use serde::{Deserialize, Serialize};
use serde_json::json;
use web_sys::{console, WebSocket, MessageEvent, ErrorEvent, CloseEvent};
use futures::future::FutureExt;
use gloo_timers::future::TimeoutFuture;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;

pub mod rete_canvas;
use crate::app::rete_canvas::ReteCanvas;

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
#[allow(dead_code)]
struct ProcessInfo {
    name: String,
    pid: u32,
    cpu_usage: f32,
    memory_usage: u64,
    status: String,
}

#[derive(Clone, Debug, Deserialize)]
#[allow(dead_code)]
struct SystemStatus {
    active_connections: usize,
    uptime: u64,
    memory_usage: f64,
    cpu_usage: f32,
    processes: Vec<ProcessInfo>,
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

#[derive(Clone)]
struct ModalPosition {
    x: i32,
    y: i32,
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
    let (filter_text, _set_filter_text) = signal(String::new());
    let (selected_level, _set_selected_level) = signal(String::from("all"));
    let (auto_scroll, _set_auto_scroll) = signal(true);
    let (position, set_position) = signal(ModalPosition { x: 100, y: 100 });
    let (is_dragging, set_is_dragging) = signal(false);
    let (drag_start, set_drag_start) = signal(ModalPosition { x: 0, y: 0 });

    let handle_mouse_down = move |ev: web_sys::MouseEvent| {
        if ev.target().unwrap().dyn_ref::<web_sys::Element>().unwrap().closest(".modal-header").is_ok() {
            set_is_dragging.set(true);
            set_drag_start.set(ModalPosition {
                x: ev.client_x() - position.get().x,
                y: ev.client_y() - position.get().y,
            });
        }
    };

    let handle_mouse_move = move |ev: web_sys::MouseEvent| {
        if is_dragging.get() {
            let start = drag_start.get();
            set_position.set(ModalPosition {
                x: ev.client_x() - start.x,
                y: ev.client_y() - start.y,
            });
        }
    };

    let handle_mouse_up = move |_| {
        set_is_dragging.set(false);
    };

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
            <div 
                class="modal logs-modal"
                on:mousedown=handle_mouse_down
                on:mousemove=handle_mouse_move
                on:mouseup=handle_mouse_up
                style:left=move || format!("{}px", position.get().x)
                style:top=move || format!("{}px", position.get().y)
            >
                <div class="modal-content logs-content">
                    <div class="modal-header">
                        <h2>"System Logs"</h2>
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
                    <div class="resize-handle"></div>
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
        cpu_usage: 0.0,
        processes: Vec::new(),
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
    let (loading_models, _set_loading_models) = signal(false);
    let (server_statuses, set_server_statuses) = signal(std::collections::HashMap::new());

    let (settings_position, set_settings_position) = signal(ModalPosition { x: 200, y: 100 });
    let (is_dragging, set_is_dragging) = signal(false);
    let (drag_start, set_drag_start) = signal(ModalPosition { x: 0, y: 0 });

    let handle_mouse_down = move |ev: web_sys::MouseEvent| {
        if ev.target().unwrap().dyn_ref::<web_sys::Element>().unwrap().closest(".modal-header").is_ok() {
            set_is_dragging.set(true);
            set_drag_start.set(ModalPosition {
                x: ev.client_x() - settings_position.get().x,
                y: ev.client_y() - settings_position.get().y,
            });
        }
    };

    let handle_mouse_move = move |ev: web_sys::MouseEvent| {
        if is_dragging.get() {
            let start = drag_start.get();
            set_settings_position.set(ModalPosition {
                x: ev.client_x() - start.x,
                y: ev.client_y() - start.y,
            });
        }
    };

    let handle_mouse_up = move |_| {
        set_is_dragging.set(false);
    };

    let toggle_settings = move |_| set_show_settings.update(|s| *s = !*s);
    
    let add_server = move |provider: &str| {
        set_config.update(|c| {
            // Only add if no server of this type exists
            if !c.servers.iter().any(|s| s.provider == provider) {
                let id = format!("{}-1", provider.to_lowercase().replace(" ", "-"));
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
            }
        });
    };

    let remove_server = move |id: String| {
        set_config.update(|c| {
            c.servers.retain(|s| s.id != id);
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
        let set_config = set_config.clone();
        let provider = server.provider.clone();
        
        // Check if already checking this server
        let current_statuses = server_statuses.get();
        if matches!(current_statuses.get(&id), Some(ConnectionStatus::Checking)) {
            return;
        }
        
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
                Ok(Ok(models)) => {
                    log!("Connection check successful for {} at {}", server.provider, server.url);
                    set_server_statuses.update(|s| { s.insert(id.clone(), ConnectionStatus::Connected); });
                    
                    // Update available models in the config
                    set_config.update(|c| {
                        // Remove existing models for this provider
                        c.available_models.retain(|m| m.provider != provider);
                        
                        // Add new models
                        c.available_models.extend(models.into_iter().map(|name| ModelConfig {
                            name,
                            provider: provider.clone(),
                        }));
                    });
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

    // Initialize servers and check connections at boot
    let _ = Effect::new(move |_| {
        // Initialize default servers if none exist
        set_config.update(|c| {
            if c.servers.is_empty() {
                c.servers = vec![
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
                ];
            }
        });

        // Check connections for all servers at boot
        spawn_local(async move {
            let config = config.get();
            for server in config.servers.iter() {
                let server_clone = server.clone();
                // Only check if not already checking or connected
                let current_status = server_statuses.get().get(&server.id).cloned();
                if !matches!(current_status, Some(ConnectionStatus::Checking | ConnectionStatus::Connected)) {
                    check_connection(server_clone);
                    // Add delay between checks
                    TimeoutFuture::new(500).await;
                }
            }
        });
    });

    // Remove the old initialization from show_settings effect
    let _ = Effect::new(move |_| {
        if show_settings.get() {
            // No need to initialize servers here anymore
            // They are initialized at boot
        }
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

    // Track connection status
    let _ = Effect::new(move |_| {
        // Register connection
        spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(&()).unwrap_or(JsValue::NULL);
            let _ = invoke_with_timeout::<()>("register_connection", args, 1000).await;
        });

        // Return cleanup function
        Box::new(move || {
            spawn_local(async move {
                let args = serde_wasm_bindgen::to_value(&()).unwrap_or(JsValue::NULL);
                let _ = invoke_with_timeout::<()>("unregister_connection", args, 1000).await;
            });
        }) as Box<dyn FnOnce()>
    });

    // Initialize WebSocket connection
    let _ = Effect::new(move |_| {
        let ws_url = config.get().ws_url.clone();
        spawn_local(async move {
            match WebSocket::new(&ws_url) {
                Ok(ws) => {
                    let onmessage_callback = Closure::wrap(Box::new(move |e: MessageEvent| {
                        if let Ok(text) = e.data().dyn_into::<js_sys::JsString>() {
                            log!("Received message: {}", text);
                        }
                    }) as Box<dyn FnMut(MessageEvent)>);
                    
                    let onerror_callback = Closure::wrap(Box::new(move |e: ErrorEvent| {
                        log!("WebSocket error: {:?}", e);
                    }) as Box<dyn FnMut(ErrorEvent)>);
                    
                    let onclose_callback = Closure::wrap(Box::new(move |e: CloseEvent| {
                        log!("WebSocket closed: {:?}", e);
                    }) as Box<dyn FnMut(CloseEvent)>);
                    
                    ws.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
                    ws.set_onerror(Some(onerror_callback.as_ref().unchecked_ref()));
                    ws.set_onclose(Some(onclose_callback.as_ref().unchecked_ref()));
                    
                    // Keep callbacks alive
                    onmessage_callback.forget();
                    onerror_callback.forget();
                    onclose_callback.forget();
                    
                    log!("WebSocket connected to {}", ws_url);
                }
                Err(e) => {
                    log!("Failed to connect to WebSocket: {:?}", e);
                }
            }
        });
    });

    view! {
        <div class="status-bar">
            <span class="status-item">"CPU: " {move || format!("{:.1}%", status.get().cpu_usage)}</span>
            <span class="status-item">"Memory: " {move || format!("{:.1}%", status.get().memory_usage)}</span>
            <span class="status-item">"Connections: " {move || status.get().active_connections}</span>
            <span class="status-item">"Uptime: " {move || status.get().uptime} "s"</span>
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
            <div 
                class="modal settings-modal"
                on:mousedown=handle_mouse_down
                on:mousemove=handle_mouse_move
                on:mouseup=handle_mouse_up
                style:left=move || format!("{}px", settings_position.get().x)
                style:top=move || format!("{}px", settings_position.get().y)
            >
                <div class="modal-content settings-content">
                    <div class="modal-header">
                        <h2>"LLM Settings"</h2>
                        <button class="close-btn" on:click=move |_| set_show_settings.set(false)>
                            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                <line x1="18" y1="6" x2="6" y2="18"/>
                                <line x1="6" y1="6" x2="18" y2="18"/>
                            </svg>
                        </button>
                    </div>
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
                        <div class="servers-section">
                            <h3>"Servers"</h3>
                            <div class="server-list">
                                {move || config.get().servers.iter().map(|server| {
                                    let server = server.clone();
                                    let server_id = server.id.clone();
                                    let server_id_for_remove = server_id.clone();
                                    view! {
                                        <div class="server-item">
                                            <ServerUrlInput
                                                id=server_id
                                                url=server.url
                                                provider=server.provider
                                                name=server.name
                                                selected_model=server.selected_model
                                                set_config=set_config
                                                server_statuses=server_statuses
                                                set_server_statuses=set_server_statuses
                                                check_connection=Box::new(check_connection.clone())
                                            />
                                            <button
                                                type="button"
                                                class="remove-server-btn"
                                                on:click=move |_| remove_server(server_id_for_remove.clone())
                                            >
                                                "Remove"
                                            </button>
                                        </div>
                                    }
                                }).collect_view()}
                            </div>
                            <div class="add-server-buttons">
                                <button
                                    type="button"
                                    on:click=move |_| add_server("LM Studio")
                                >
                                    "Add LM Studio Server"
                                </button>
                                <button
                                    type="button"
                                    on:click=move |_| add_server("Ollama")
                                >
                                    "Add Ollama Server"
                                </button>
                            </div>
                        </div>
                        <div class="settings-actions">
                            <button type="submit">"Save Settings"</button>
                            <button type="button" on:click=move |_| set_show_settings.set(false)>
                                "Cancel"
                            </button>
                        </div>
                    </form>
                    <div class="resize-handle"></div>
                </div>
            </div>
        })}
    }
}

#[component]
pub fn App() -> impl IntoView {
    view! {
        <div class="app-container">
            <ReteCanvas/>
            <StatusBar/>
        </div>
    }
}
