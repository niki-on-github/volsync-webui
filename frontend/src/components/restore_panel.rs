use crate::api::trigger_restore;
use crate::api::Snapshot;
use chrono::Utc;
use yew::prelude::*;

#[derive(Properties, Clone, PartialEq)]
pub struct Props {
    pub app_name: String,
    pub ns: String,
    pub snapshots: Vec<Snapshot>,
}

#[function_component]
pub fn RestorePanel(props: &Props) -> Html {
    let timestamp: UseStateHandle<String> = use_state(String::new);
    let restoring: UseStateHandle<bool> = use_state(|| false);
    let status: UseStateHandle<String> = use_state(|| "Ready".to_string());

    let on_change = {
        let timestamp = timestamp.clone();
        Callback::from(move |e: Event| {
            let val = e.target_unchecked_into::<web_sys::HtmlSelectElement>().value();
            timestamp.set(val);
        })
    };

    let on_restore = {
        let ts = (*timestamp).clone();
        let app = props.app_name.clone();
        let ns = props.ns.clone();
        let status = status.clone();
        let restoring = restoring.clone();
        Callback::from(move |_| {
            if ts.is_empty() {
                status.set("Please select a snapshot timestamp".to_string());
                return;
            }
            restoring.set(true);
            status.set("Starting restore...".to_string());
            let trigger = format!("restore-{}", Utc::now().format("%Y%m%d-%H%M%S"));
            let app_clone = app.clone();
            let ns_clone = ns.clone();
            let status_clone = status.clone();
            let restoring_clone = restoring.clone();
            let ts_clone = ts.clone();
            wasm_bindgen_futures::spawn_local(async move {
                // Pass None when timestamp is empty (means "latest")
                let ts_opt = if ts_clone.is_empty() { None } else { Some(ts_clone) };
                match trigger_restore(&app_clone, &ns_clone, &trigger, ts_opt).await {
                    Ok(r) => {
                        restoring_clone.set(false);
                        status_clone.set(
                            if r.result.as_deref() == Some("Successful") {
                                "Restore completed successfully".to_string()
                            } else {
                                format!("Restore completed with result: {:?}", r.result)
                            }
                        );
                    }
                    Err(e) => {
                        restoring_clone.set(false);
                        status_clone.set(format!("Restore error: {}", e));
                    }
                }
            });
        })
    };

    html! {
        <div class="bg-gray-800 rounded-lg p-4">
            <h3 class="text-lg font-medium text-white mb-4">{"Restore"}</h3>
            <div class="mb-4">
                <label class="block text-sm font-medium text-gray-300 mb-2">
                    {"Select Snapshot (RFC3339 timestamp)"}
                </label>
                <select
                    class="w-full bg-gray-700 border border-gray-600 rounded-md px-3 py-2 text-white text-sm focus:outline-none focus:ring-2 focus:ring-cyan-500"
                    value={(*timestamp).clone()}
                    onchange={on_change}
                >
                    <option value="">{"Latest (no timestamp)"}</option>
                    {props.snapshots.iter().map(|snap| {
                        let label = format!("{} - {}", snap.id, snap.time);
                        html! { <option key={snap.id.clone()} value={snap.time.clone()}>{label}</option> }
                    }).collect::<Html>()}
                </select>
            </div>
            <button
                onclick={on_restore}
                disabled={*restoring || props.app_name.is_empty()}
                class="px-4 py-2 bg-yellow-600 hover:bg-yellow-700 disabled:bg-gray-600 text-white rounded-md transition-colors"
            >
                {if *restoring { "Restoring..." } else { "Restore" }}
            </button>
            <p class="text-sm text-gray-300 mt-3">{"Status: "}{(*status).clone()}</p>
        </div>
    }
}