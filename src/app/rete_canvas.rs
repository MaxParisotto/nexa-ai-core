use leptos::*;
use leptos::html::*;
use leptos::task::spawn_local;
use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use web_sys;
use serde_json;
use gloo_console as log;
use std::sync::{Arc, Mutex};
use gloo_net::websocket::{WebSocket, WebSocketError};
use futures::StreamExt;

#[wasm_bindgen]
extern "C" {
    type ReteEditor;

    #[wasm_bindgen(constructor)]
    fn new(container: web_sys::HtmlElement) -> ReteEditor;

    #[wasm_bindgen(method)]
    fn addNode(this: &ReteEditor) -> js_sys::Promise;

    #[wasm_bindgen(method)]
    fn clear(this: &ReteEditor);

    #[wasm_bindgen(method)]
    fn destroy(this: &ReteEditor);

    #[wasm_bindgen(method)]
    fn getNodeData(this: &ReteEditor, node_id: &str) -> js_sys::Promise;

    #[wasm_bindgen(method)]
    fn updateNodeData(this: &ReteEditor, node_id: &str, data: &JsValue) -> js_sys::Promise;
}

#[derive(Clone, Debug)]
struct NodeState {
    id: String,
    loading: bool,
    error: Option<String>,
    data: Option<serde_json::Value>,
}

#[derive(Clone)]
struct WebSocketWrapper(Rc<RefCell<Option<web_sys::WebSocket>>>);

impl WebSocketWrapper {
    fn new() -> Self {
        Self(Rc::new(RefCell::new(None)))
    }

    fn set(&self, ws: web_sys::WebSocket) {
        *self.0.borrow_mut() = Some(ws);
    }

    fn close(&self) {
        if let Some(ws) = self.0.borrow_mut().take() {
            let _ = ws.close();
        }
    }
}

#[component]
fn StatusIndicator(
    id: String,
    loading: bool,
    error: Option<String>,
) -> impl IntoView {
    let (status_text, set_status_text) = create_signal(String::new());
    let (status_class, set_status_class) = create_signal(String::new());

    create_effect(move |_| {
        let text = if loading {
            "Processing...".to_string()
        } else if let Some(err) = error.clone() {
            err
        } else {
            "Ready".to_string()
        };
        set_status_text.set(text);

        let class = if loading {
            "loading-indicator"
        } else if error.is_some() {
            "error-message"
        } else {
            "success-indicator"
        };
        set_status_class.set(class.to_string());
    });

    view! {
        <div class="node-status">
            <span class="node-id">{move || id.clone()}</span>
            <span class={move || status_class.get()}>
                {move || status_text.get()}
            </span>
        </div>
    }
}

#[derive(Clone)]
struct EditorState {
    editor: std::rc::Rc<std::cell::RefCell<Option<ReteEditor>>>,
}

#[component]
pub fn ReteCanvas() -> impl IntoView {
    let editor_state = EditorState {
        editor: std::rc::Rc::new(std::cell::RefCell::new(None)),
    };
    
    let (nodes, set_nodes) = create_signal(Vec::<NodeState>::new());
    let (loading, set_loading) = create_signal(false);
    let (error, set_error) = create_signal(None::<String>);
    let ws = Arc::new(Mutex::new(None::<WebSocket>));

    let editor_state_clone = editor_state.clone();
    let add_node = move |_| {
        log::log!("Add Node button clicked");
        let editor_ref = editor_state_clone.editor.borrow();
        if let Some(editor_instance) = editor_ref.as_ref() {
            set_loading.set(true);
            set_error.set(None);

            let promise = editor_instance.addNode();
            let set_loading = set_loading.clone();
            let set_error = set_error.clone();
            let set_nodes = set_nodes.clone();
            
            spawn_local(async move {
                match wasm_bindgen_futures::JsFuture::from(promise).await {
                    Ok(result) => {
                        if let Ok(node_data) = serde_wasm_bindgen::from_value::<serde_json::Value>(result) {
                            let node_id = node_data["id"].as_str().unwrap_or_default().to_string();
                            set_nodes.update(|nodes| {
                                nodes.push(NodeState {
                                    id: node_id,
                                    loading: false,
                                    error: None,
                                    data: Some(node_data),
                                });
                            });
                            log::log!("Node added successfully");
                        }
                        set_loading.set(false);
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to add node: {:?}", e);
                        log::error!("{}", &error_msg);
                        set_error.set(Some(error_msg));
                        set_loading.set(false);
                    }
                }
            });
        } else {
            let error_msg = "Editor not initialized";
            log::error!("{}", error_msg);
            set_error.set(Some(error_msg.to_string()));
        }
    };

    let editor_state_clone = editor_state.clone();
    let clear_canvas = move |_| {
        log::info!("Clear button clicked");
        if let Some(editor_instance) = editor_state_clone.editor.borrow().as_ref() {
            editor_instance.clear();
            set_nodes.set(Vec::new());
            log::info!("Canvas cleared successfully");
        } else {
            let error_msg = "Editor not initialized";
            log::error!("{}", error_msg);
            set_error.set(Some(error_msg.to_string()));
        }
    };

    let setup_websocket = {
        let ws = ws.clone();
        let set_error = set_error.clone();
        let set_nodes = set_nodes.clone();
        
        move || {
            spawn_local(async move {
                match WebSocket::open("ws://127.0.0.1:9001") {
                    Ok(new_ws) => {
                        log::info!("WebSocket connection opened");
                        let (_, mut read) = new_ws.split();
                        
                        if let Ok(mut guard) = ws.lock() {
                            *guard = Some(new_ws);
                        }
                        
                        while let Some(msg) = read.next().await {
                            match msg {
                                Ok(message) => {
                                    if let Ok(text) = message.text() {
                                        if let Ok(data) = serde_json::from_str::<serde_json::Value>(&text) {
                                            handle_ws_message(data, &set_nodes);
                                        }
                                    }
                                }
                                Err(e) => {
                                    let error_msg = format!("WebSocket error: {:?}", e);
                                    log::error!("{}", &error_msg);
                                    set_error.set(Some(error_msg));
                                    break;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to connect to WebSocket: {:?}", e);
                        log::error!("{}", &error_msg);
                        set_error.set(Some(error_msg));
                    }
                }
            });
        }
    };

    let editor_state_clone = editor_state.clone();
    let ws_clone = ws.clone();
    
    create_effect(move |_| {
        log::info!("Initializing ReteEditor");
        if let Some(window) = web_sys::window() {
            if let Some(document) = window.document() {
                if let Some(container) = document.get_element_by_id("rete") {
                    if let Ok(html_container) = container.dyn_into::<web_sys::HtmlElement>() {
                        *editor_state_clone.editor.borrow_mut() = Some(ReteEditor::new(html_container));
                        log::info!("ReteEditor initialized successfully");
                        setup_websocket();
                    }
                }
            }
        }

        on_cleanup(move || {
            if let Ok(mut guard) = ws_clone.lock() {
                if let Some(ws) = guard.take() {
                    ws.close();
                }
            }
        });
    });

    view! {
        <div class="app-container">
            <div class="canvas-container">
                <div id="rete" style="width: 100%; height: 100%; position: relative;"></div>
                <div class="editor-controls">
                    <button
                        on:click=add_node
                        disabled=move || loading.get()
                        class="editor-button"
                    >
                        {move || if loading.get() { "Adding..." } else { "Add Node" }}
                    </button>
                    <button
                        on:click=clear_canvas
                        disabled=move || loading.get()
                        class="editor-button"
                    >
                        "Clear"
                    </button>
                </div>
                {move || error.get().map(|err| view! {
                    <div class="error-message">
                        {err}
                    </div>
                })}
                <div class="nodes-container">
                    {move || nodes.get().into_iter().map(|node| {
                        view! {
                            <StatusIndicator
                                id=node.id
                                loading=node.loading
                                error=node.error
                            />
                        }
                    }).collect_view()}
                </div>
            </div>
        </div>
    }
}

fn handle_ws_message(data: serde_json::Value, nodes_signal: &WriteSignal<Vec<NodeState>>) {
    if let Some(msg_type) = data.get("type").and_then(|t| t.as_str()) {
        match msg_type {
            "node_update" => {
                if let Some(node_data) = data.get("data") {
                    if let Some(node_id) = node_data.get("id").and_then(|id| id.as_str()) {
                        nodes_signal.update(|nodes| {
                            if let Some(node) = nodes.iter_mut().find(|n| n.id == node_id) {
                                node.loading = false;
                                node.error = None;
                                node.data = Some(node_data.clone());
                            } else {
                                nodes.push(NodeState {
                                    id: node_id.to_string(),
                                    loading: false,
                                    error: None,
                                    data: Some(node_data.clone()),
                                });
                            }
                        });
                    }
                }
            }
            "node_error" => {
                if let Some(error_data) = data.get("data") {
                    if let Some(node_id) = error_data.get("id").and_then(|id| id.as_str()) {
                        nodes_signal.update(|nodes| {
                            if let Some(node) = nodes.iter_mut().find(|n| n.id == node_id) {
                                node.loading = false;
                                node.error = error_data.get("error")
                                    .and_then(|e| e.as_str())
                                    .map(|e| e.to_string());
                            }
                        });
                    }
                }
            }
            _ => log::warn!("Unknown message type: {}", msg_type)
        }
    }
}