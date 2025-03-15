#![windows_subsystem = "windows"]

use chrono::{DateTime, Local};
use civ7_mod_manager::{ModDirEntry, ModManager, ModSpec};
use dioxus::prelude::*;
use std::{collections::HashSet, sync::Arc};

use async_callback::*;
use components::{AddModDialog, ModDirInfo, ModList};
use dialog::*;

mod async_callback;
mod components;
mod dialog;

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let mut manager = use_context_provider(|| {
        SyncSignal::new_maybe_sync(Arc::new(ModManager::load_default().unwrap()))
    });

    let root_dir = manager.read().root_dir().display().to_string();

    let mut entries = use_resource(move || async move {
        let dirs = manager
            .read()
            .list_dirs()
            .await
            .map_err(dioxus::CapturedError::from_display)?
            .into_iter()
            .filter_map(|dir| {
                let (entry, dirname, spec) = match dir {
                    ModDirEntry::Managed(entry, dirname, spec) => (entry, dirname, Some(spec)),
                    ModDirEntry::Unmanaged(entry, dirname) => (entry, dirname, None),
                };

                let content_size =
                    dir_size::get_size_in_human_bytes(&entry.path()).unwrap_or_default();

                let last_updated = entry
                    .metadata()
                    .and_then(|m| m.modified())
                    .map(|system_time| {
                        DateTime::<Local>::from(system_time)
                            .format("%F %T %Z")
                            .to_string()
                    })
                    .unwrap_or_default();

                let entry = if let Some(spec) = spec {
                    ModDirInfo::Managed {
                        dirname,
                        content_size,
                        last_updated,
                        spec,
                        loading: false,
                    }
                } else {
                    ModDirInfo::Unmanaged {
                        dirname,
                        content_size,
                        last_updated,
                    }
                };

                Some(entry)
            });
        dioxus::Ok(dirs.collect::<Vec<_>>())
    });

    let mut refresh = move || entries.restart();

    let mut loadings = use_signal_sync(HashSet::<String>::new);

    let entries: Vec<_> = entries.suspend()?.read().to_owned()?;
    let entries = use_memo(use_reactive!(|entries| {
        let mut entries = entries;
        for entry in &mut entries {
            entry.set_loading(loadings.read().contains(entry.dirname().as_str()));
        }
        entries
    }));

    let update = move |dirname: String, spec: ModSpec| async move {
        loadings.write().insert(dirname.clone());
        let result = manager
            .read_unchecked()
            .install_or_update(dirname.clone(), spec)
            .await;
        loadings.write().remove(&dirname);
        match result {
            Ok(provision) => {
                dbg!(provision);
            }
            Err(err) => {
                dbg!(err);
            }
        }
    };

    let update_all = move |_| {
        spawn(async move {
            let tasks = tokio::task::LocalSet::new();
            for entry in entries() {
                if let ModDirInfo::Managed { dirname, spec, .. } = entry {
                    tasks.spawn_local(update(dirname, spec));
                }
            }
            tasks.await;
            manager.write();
        });
    };

    let onupdate = move |(dirname, spec): (String, ModSpec)| async move {
        let _ = spawn(update(dirname, spec));
        anyhow::Ok(())
    };

    let onremove = move |dirname: String| async move {
        spawn(async move {
            loadings.write().insert(dirname.clone());
            let result = manager.write().uninstall(&dirname).await;
            match result {
                Ok(spec) => {
                    dbg!(spec);
                }
                Err(err) => {
                    dbg!(err);
                }
            }
            loadings.write().remove(&dirname);
        });
        Ok(())
    };

    let mut add_mod_dialog = use_dialog(|state| {
        let onsubmit = move |(dirname, spec)| async move {
            manager.write().install_or_update(dirname, spec).await?;
            Ok(())
        };
        rsx! {
            AddModDialog { state, onsubmit }
        }
    });

    rsx! {
        document::Title { "Civilization VII Mod Manager" }
        document::Stylesheet { href: asset!("/assets/tailwind.css") }

        div { class: "flex flex-wrap justify-between items-center gap-4 m-4",
            div { class: "flex-auto",
                h1 { class: "text-2xl font-bold", "Civilization VII Mod Manager" }
                div { class: "text-sm", {root_dir} }
            }


            button { class: "btn btn-ghost", onclick: move |_| refresh(), "Refresh" }

            div { class: "join flex-auto",
                button {
                    class: "join-item flex-auto btn btn-soft [--btn-color:var(--color-blue-500)]",
                    onclick: move |_| { add_mod_dialog.state.open.set(true) },
                    "Add"
                }
                button {
                    class: "join-item flex-auto btn btn-soft [--btn-color:var(--color-green-500)]",
                    onclick: update_all,
                    "Update All"
                }
            }
        }

        div { class: "divider m-0" }

        ModList { entries, onremove, onupdate }

        {add_mod_dialog.element}
    }
}
