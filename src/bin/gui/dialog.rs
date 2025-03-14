use dioxus::prelude::*;

pub struct Dialog {
    pub element: Element,
    pub state: DialogState,
}

#[derive(Clone, Copy, PartialEq)]
pub struct DialogState {
    pub open: Signal<bool>,
    pub loading: Signal<bool>,
    pub error_message: Signal<Option<String>>,
}

pub fn use_dialog(mut content: impl FnMut(DialogState) -> Element) -> Dialog {
    let mut open = use_signal(|| false);
    let loading = use_signal(|| false);
    let error_message = use_signal(|| None);
    let state = DialogState {
        open,
        loading,
        error_message,
    };

    let content = content(state);
    let element = rsx! {
        dialog { class: "modal", open: open(),
            div { class: "modal-box", {content} }
            div { class: "modal-backdrop",
                button { onclick: move |_| { open.set(false) } }
            }
        }
    };

    Dialog { element, state }
}
