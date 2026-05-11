use crate::api::{self, App, Snapshot};
use crate::components::app_selector::AppSelector;
use crate::components::backup_panel::BackupPanel;
use crate::components::namespace::NamespaceSelector;
use crate::components::restore_panel::RestorePanel;
use crate::components::snapshot_list::SnapshotList;
use yew::prelude::*;

#[function_component]
pub fn AppComponent() -> Html {
    let namespaces: UseStateHandle<Vec<String>> = use_state(Vec::new);
    let apps: UseStateHandle<Vec<App>> = use_state(Vec::new);
    let selected_app: UseStateHandle<Option<App>> = use_state(|| None);
    let snapshots: UseStateHandle<Vec<Snapshot>> = use_state(Vec::new);
    let loading_snapshots: UseStateHandle<bool> = use_state(|| false);
    // Bug #6 fix: Track selected namespace for filtering
    let selected_namespace: UseStateHandle<String> = use_state(String::new);

    {
        let namespaces_clone = namespaces.clone();
        use_effect_with((), |_| {
            wasm_bindgen_futures::spawn_local(async move {
                match api::list_namespaces().await {
                    Ok(ns) => namespaces_clone.set(ns),
                    Err(e) => log::error!("Failed to load namespaces: {}", e),
                }
            });
        });
    }

    // Bug #6 fix: Reload apps when namespace changes
    {
        let apps_clone = apps.clone();
        use_effect_with(
            (*selected_namespace).clone(),
            move |ns: &String| {
                let apps = apps_clone.clone();
                let ns_owned = ns.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    let ns_param = if ns_owned.is_empty() { None } else { Some(ns_owned.as_str()) };
                    match api::list_apps(ns_param).await {
                        Ok(a) => apps.set(a),
                        Err(e) => log::error!("Failed to load apps: {}", e),
                    }
                });
            },
        );
    }

    {
        let snapshots_clone = snapshots.clone();
        let loading_clone = loading_snapshots.clone();
        let app_clone = (*selected_app).clone();
        use_effect_with(
            app_clone,
            move |app| {
                if let Some(ref a) = app {
                    loading_clone.set(true);
                    let name = a.name.clone();
                    let ns = a.namespace.clone();
                    let snaps = snapshots_clone.clone();
                    let load = loading_clone.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        match api::get_snapshots(&name, &ns).await {
                            Ok(s) => {
                                snaps.set(s);
                                load.set(false);
                            }
                            Err(e) => {
                                snaps.set(Vec::new());
                                load.set(false);
                                log::warn!("Failed to load snapshots: {}", e);
                            }
                        }
                    });
                }
            },
        );
    }

    let on_app_select = {
        let selected_app = selected_app.clone();
        Callback::from(move |app: Option<App>| selected_app.set(app))
    };

    // Bug #6 fix: Wire up namespace selector callback
    let on_namespace_select = {
        let selected_ns = selected_namespace.clone();
        Callback::from(move |ns: String| selected_ns.set(ns))
    };

    let on_refresh = {
        let snapshots = snapshots.clone();
        let loading = loading_snapshots.clone();
        let app = (*selected_app).clone();
        Callback::from(move |_| {
            if let Some(ref a) = app {
                loading.set(true);
                let name = a.name.clone();
                let ns = a.namespace.clone();
                let snaps = snapshots.clone();
                let load = loading.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match api::get_snapshots(&name, &ns).await {
                        Ok(s) => {
                            snaps.set(s);
                            load.set(false);
                        }
                        Err(e) => {
                            snaps.set(Vec::new());
                            load.set(false);
                            log::warn!("Failed to refresh snapshots: {}", e);
                        }
                    }
                });
            }
        })
    };

    html! {
        <div class="min-h-screen bg-gray-900 text-white">
            <header class="bg-gray-800 border-b border-gray-700 px-6 py-4">
                <div class="flex items-center justify-between">
                    <h1 class="text-xl font-bold text-cyan-400">{"Volsync WebUI"}</h1>
                    <div class="flex items-center gap-4">
                        <NamespaceSelector
                            selected={(*selected_namespace).clone()}
                            namespaces={(*namespaces).clone()}
                            on_select={on_namespace_select}
                        />
                        <AppSelector
                            selected={(*selected_app).clone()}
                            apps={(*apps).clone()}
                            on_select={on_app_select}
                        />
                    </div>
                </div>
            </header>

            <main class="p-6">
                if let Some(ref app) = *selected_app {
                    <div class="grid grid-cols-1 lg:grid-cols-3 gap-6">
                        <div>
                            <SnapshotList
                                snapshots={(*snapshots).clone()}
                                loading={*loading_snapshots}
                                on_refresh={on_refresh}
                            />
                        </div>
                        <div>
                            <BackupPanel
                                app_name={app.name.clone()}
                                ns={app.namespace.clone()}
                            />
                        </div>
                        <div>
                            <RestorePanel
                                app_name={app.name.clone()}
                                ns={app.namespace.clone()}
                                snapshots={(*snapshots).clone()}
                            />
                        </div>
                    </div>
                } else {
                    <div class="flex items-center justify-center h-64">
                        <p class="text-gray-400 text-lg">{"Select an application to manage backups"}</p>
                    </div>
                }
            </main>
        </div>
    }
}