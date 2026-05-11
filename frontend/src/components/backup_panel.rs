use crate::api::{trigger_backup, trigger_backup_all, BackupAllResponse};
use yew::prelude::*;

#[derive(Properties, Clone, PartialEq)]
pub struct Props {
    pub app_name: String,
    pub ns: String,
}

#[function_component]
pub fn BackupPanel(props: &Props) -> Html {
    let status: UseStateHandle<String> = use_state(|| "Ready".to_string());
    let backing_up: UseStateHandle<bool> = use_state(|| false);
    let backing_up_all: UseStateHandle<bool> = use_state(|| false);
    let backup_all_results: UseStateHandle<Option<BackupAllResponse>> = use_state(|| None);

    let on_backup = {
        let status = status.clone();
        let backing_up = backing_up.clone();
        let app = props.app_name.clone();
        let ns = props.ns.clone();
        Callback::from(move |_| {
            backing_up.set(true);
            status.set("Starting backup...".to_string());
            let app_clone = app.clone();
            let ns_clone = ns.clone();
            let status_clone = status.clone();
            let backing_up_clone = backing_up.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match trigger_backup(&app_clone, &ns_clone).await {
                    Ok(r) => {
                        backing_up_clone.set(false);
                        status_clone.set(
                            if r.result.as_deref() == Some("Successful") {
                                "Backup completed successfully".to_string()
                            } else {
                                format!("Backup failed: {:?}", r.result)
                            }
                        );
                    }
                    Err(e) => {
                        backing_up_clone.set(false);
                        status_clone.set(format!("Backup error: {}", e));
                    }
                }
            });
        })
    };

    let on_backup_all = {
        let status = status.clone();
        let backing_up_all = backing_up_all.clone();
        let results = backup_all_results.clone();
        Callback::from(move |_| {
            backing_up_all.set(true);
            status.set("Starting backup for all apps...".to_string());
            let status_clone = status.clone();
            let backing_up_all_clone = backing_up_all.clone();
            let results_clone = results.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match trigger_backup_all().await {
                    Ok(r) => {
                        backing_up_all_clone.set(false);
                        let failed: Vec<_> = r.apps.iter().filter(|a| !a.success).collect();
                        let msg = if failed.is_empty() {
                            "All backups completed successfully".to_string()
                        } else {
                            format!("{} of {} backups failed", failed.len(), r.apps.len())
                        };
                        status_clone.set(msg);
                        results_clone.set(Some(r));
                    }
                    Err(e) => {
                        backing_up_all_clone.set(false);
                        status_clone.set(format!("Backup-all error: {}", e));
                    }
                }
            });
        })
    };

    html! {
        <div class="bg-gray-800 rounded-lg p-4">
            <h3 class="text-lg font-medium text-white mb-4">{"Backup"}</h3>
            <div class="flex gap-3 mb-4">
                <button
                    onclick={on_backup.clone()}
                    disabled={*backing_up || props.app_name.is_empty()}
                    class="px-4 py-2 bg-cyan-600 hover:bg-cyan-700 disabled:bg-gray-600 text-white rounded-md transition-colors"
                >
                    {if *backing_up { "Backing up..." } else { "Backup" }}
                </button>
                <button
                    onclick={on_backup_all}
                    disabled={*backing_up_all}
                    class="px-4 py-2 bg-green-600 hover:bg-green-700 disabled:bg-gray-600 text-white rounded-md transition-colors"
                >
                    {if *backing_up_all { "Processing..." } else { "Backup All Apps" }}
                </button>
            </div>
            <p class="text-sm text-gray-300">{"Status: "}{(*status).clone()}</p>

            if let Some(ref results) = *backup_all_results {
                <div class="mt-4">
                    <h4 class="text-sm font-medium text-gray-300 mb-2">{"Results:"}</h4>
                    <div class="space-y-1">
                        {results.apps.iter().map(|r| {
                            html! {
                                <div class="flex items-center gap-2 text-sm">
                                    <span class={if r.success { "text-green-400" } else { "text-red-400" }}>
                                        {if r.success { "✓" } else { "✗" }}
                                    </span>
                                    <span class="text-white">{&r.app}</span>
                                    <span class="text-gray-400">{"("}{&r.namespace}{")"}</span>
                                    if let Some(err) = &r.error {
                                        <span class="text-red-400 text-xs">{err}</span>
                                    }
                                </div>
                            }
                        }).collect::<Html>()}
                    </div>
                </div>
            }
        </div>
    }
}