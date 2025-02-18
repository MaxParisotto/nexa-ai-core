use leptos::*;
use leptos::html::*;
use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use web_sys;

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
}

#[component]
pub fn ReteCanvas() -> impl IntoView {
    let editor: std::rc::Rc<std::cell::RefCell<Option<ReteEditor>>> = std::rc::Rc::new(std::cell::RefCell::new(None));
    let editor_for_effect = editor.clone();
    let editor_for_commands = editor.clone();
    let editor_for_clear = editor.clone();

    // Initialize editor when component mounts
    Effect::new(move |_| {
        if let Some(container) = document()
            .get_element_by_id("rete")
            .and_then(|el| el.dyn_into::<web_sys::HtmlElement>().ok())
        {
            *editor_for_effect.borrow_mut() = Some(ReteEditor::new(container));
        }
    });

    view! {
        <div class="app-container">
            <div class="canvas-container">
                <div id="rete"></div>
                <div class="editor-controls">
                    <button on:click=move |_| {
                        if let Some(editor_ref) = editor_for_commands.borrow().as_ref() {
                            let _ = editor_ref.addNode();
                        }
                    }>
                        "Add Node"
                    </button>
                    <button on:click=move |_| {
                        if let Some(editor_ref) = editor_for_clear.borrow().as_ref() {
                            editor_ref.clear();
                        }
                    }>
                        "Clear"
                    </button>
                </div>
            </div>
        </div>
    }
}