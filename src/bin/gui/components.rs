use crate::{use_dialog, AsyncCallback, DialogState};
use civ7_mod_manager::{provider::Civfanatics, ModSpec};
use dioxus::prelude::*;
use std::str::FromStr;

#[derive(Clone, PartialEq)]
pub enum ModDirInfo {
    Managed {
        dirname: String,
        content_size: String,
        last_updated: String,
        spec: ModSpec,
        loading: bool,
    },
    Unmanaged {
        dirname: String,
        content_size: String,
        last_updated: String,
    },
}

impl ModDirInfo {
    pub const fn dirname(&self) -> &String {
        match self {
            Self::Managed { dirname, .. } => dirname,
            Self::Unmanaged { dirname, .. } => dirname,
        }
    }

    pub const fn content_size(&self) -> &String {
        match self {
            Self::Managed { content_size, .. } => content_size,
            Self::Unmanaged { content_size, .. } => content_size,
        }
    }

    pub const fn last_updated(&self) -> &String {
        match self {
            Self::Managed { last_updated, .. } => last_updated,
            Self::Unmanaged { last_updated, .. } => last_updated,
        }
    }

    pub const fn loading(&self) -> bool {
        match self {
            Self::Managed { loading, .. } => *loading,
            _ => false,
        }
    }

    pub const fn set_loading(&mut self, loading: bool) {
        match self {
            Self::Managed {
                loading: loading_, ..
            } => {
                *loading_ = loading;
            }
            _ => {}
        }
    }

    pub const fn spec(&self) -> Option<&ModSpec> {
        match self {
            Self::Managed { spec, .. } => Some(spec),
            _ => None,
        }
    }
}

#[component]
pub fn ModList(
    entries: ReadOnlySignal<Vec<ModDirInfo>>,
    #[props(into)] onremove: ReadOnlySignal<AsyncCallback<String, anyhow::Result<()>>>,
    #[props(into)] onupdate: ReadOnlySignal<AsyncCallback<(String, ModSpec), anyhow::Result<()>>>,
) -> Element {
    let items = entries().into_iter().map(|entry| {
        let dirname_ = entry.dirname().clone();
        let onremove = move |_| onremove.read().call(dirname_.clone());

        let onupdate = if let Some(spec) = entry.spec().cloned() {
            let dirname = entry.dirname().clone();
            Some(AsyncCallback::new(move |_| {
                onupdate.read().call((dirname.clone(), spec.clone()))
            }))
        } else {
            None
        };

        #[allow(unused_variables)]
        let key = entry.dirname().clone();

        rsx! {
            ModListItem {
                key,
                entry,
                onremove,
                onupdate,
            }
        }
    });

    rsx! {
        ul { class: "list", {items} }
    }
}

#[component]
pub fn ModListItem(
    entry: ReadOnlySignal<ModDirInfo>,
    #[props(into)] onremove: ReadOnlySignal<AsyncCallback<(), anyhow::Result<()>>>,
    #[props(into)] onupdate: ReadOnlySignal<Option<AsyncCallback<(), anyhow::Result<()>>>>,
) -> Element {
    let mut dialog_for_remove = use_dialog(move |mut state| {
        let onsubmit = move |_| async move {
            state.loading.set(true);
            let result = onremove.read().call(()).await;
            state.loading.set(false);
            match result {
                Ok(_) => {
                    state.error_message.set(None);
                    state.open.set(false);
                }
                Err(err) => state.error_message.set(Some(err.to_string())),
            }
            Ok(())
        };

        rsx! {
            RemoveModDialog { state, dirname: entry().dirname(), onsubmit }
        }
    });

    rsx! {
        li { class: "list-row",
            div { class: "list-col-grow",
                div { class: "text-lg",
                    span { class: "mr-3", {entry().dirname().clone()} }
                    if entry().loading() {
                        span { class: "loading loading-sm loading-spinner" }
                    }
                }
                div { class: "text-xs",
                    {entry().content_size().clone()}
                    " · "
                    {entry().last_updated().clone()}
                }
                div { class: "text-xs",
                    match entry().spec() {
                        Some(spec) if spec.source == "civfanatics" => rsx! {
                            a {
                                class: "link link-info",
                                href: Civfanatics::default().page_url(&spec.identifier),
                                target: "_blank",
                                {spec.to_string()}
                            }
                        },
                        _ => rsx! {
                            span { class: "text-zinc-500",
                                "This directory is not tracked by this tool. If you want to manage it, please add it via the Mod Manager."
                            }
                        },
                    }
                }
            }
            div { class: " join items-center",
                button {
                    class: "join-item btn btn-sm btn-soft [--btn-color:var(--color-rose-400)]",
                    r#type: "button",
                    onclick: move |_| dialog_for_remove.state.open.set(true),
                    "Remove"
                }
                if entry().spec().is_some() {
                    button {
                        class: "join-item btn btn-sm btn-soft [--btn-color:var(--color-green-400)]",
                        r#type: "button",
                        onclick: move |_| {
                            spawn(async move {
                                if let Some(onupdate) = onupdate.read().as_ref() {
                                    onupdate.call(()).await.ok();
                                }
                            });
                        },
                        "Update"
                    }
                }
            }
            {dialog_for_remove.element}
        }
    }
}

#[component]
pub fn AddModDialog(
    state: DialogState,
    #[props(into)] onsubmit: ReadOnlySignal<AsyncCallback<(String, ModSpec), anyhow::Result<()>>>,
) -> Element {
    let mut open = state.open;
    let mut loading = state.loading;
    let mut error_message = state.error_message;

    let mut mod_id = use_signal(String::new);
    let resolved_spec = use_memo(move || {
        ModSpec::from_str(&mod_id())
            .and_then(|spec| Ok((spec.resolve_dirname()?, spec)))
            .ok()
    });

    let onsubmit = move |evt: FormEvent| {
        evt.prevent_default();
        if let Some((dirname, spec)) = resolved_spec().as_ref().cloned() {
            spawn(async move {
                loading.set(true);
                let result = onsubmit.read().call((dirname, spec)).await;
                loading.set(false);
                match result {
                    Ok(_) => {
                        mod_id.set(String::new());
                        error_message.set(None);
                        open.set(false);
                    }
                    Err(err) => error_message.set(Some(err.to_string())),
                }
            });
        } else {
            error_message.set(Some("Invalid Mod ID".to_string()));
        }
    };

    rsx! {
        form { onsubmit,
            fieldset { class: "fieldset",
                legend { class: "fieldset-legend text-lg font-bold pt-0", "Add Mod" }
                input {
                    class: "input w-full",
                    r#type: "text",
                    placeholder: "Civfanatics Resource ID",
                    value: mod_id,
                    oninput: move |evt| { mod_id.set(evt.value()) },
                }
                p { class: "fieldset-label",
                    span { "e.g. sukritacts-simple-ui-adjustments.31860" }
                }
                div { class: "join w-full flex",
                    button {
                        class: "join-item flex-1 btn",
                        r#type: "button",
                        onclick: move |_| open.set(false),
                        "Cancel"
                    }
                    button {
                        class: "join-item flex-1 btn btn-primary",
                        r#type: "submit",
                        disabled: resolved_spec().is_none(),
                        if loading() {
                            span { class: "loading loading-spinner" }
                        }
                        "Add"
                    }
                }
                div {
                    if let Some(msg) = error_message() {
                        div { class: "text-red-500", "{msg}" }
                    }
                }
            }
        }
    }
}

#[component]
pub fn RemoveModDialog(
    state: DialogState,
    dirname: ReadOnlySignal<String>,
    #[props(into)] onsubmit: ReadOnlySignal<AsyncCallback<(), anyhow::Result<()>>>,
) -> Element {
    let mut open = state.open;
    let mut loading = state.loading;
    let mut error_message = state.error_message;

    let onsubmit = move |_| {
        spawn(async move {
            loading.set(true);
            let result = onsubmit.read().call(()).await;
            loading.set(false);
            match result {
                Ok(_) => open.set(false),
                Err(err) => error_message.set(Some(err.to_string())),
            }
        });
    };

    rsx! {
        p {
            "Are you sure you want to remove "
            strong { {dirname} }
            "?"
        }
        div { class: "modal-action",
            button {
                class: "btn [--btn-color:var(--color-rose-600)]",
                onclick: onsubmit,
                r#type: "button",
                "Remove"
            }
            form {
                method: "dialog",
                onsubmit: move |evt| {
                    evt.prevent_default();
                    open.set(false);
                },
                button { class: "btn btn-neutral", r#type: "submit", "Cancel" }
            }
        }
    }
}
