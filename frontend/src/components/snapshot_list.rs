use crate::api::Snapshot;
use yew::prelude::*;

#[derive(Properties, Clone, PartialEq)]
pub struct Props {
    pub snapshots: Vec<Snapshot>,
    pub loading: bool,
    pub on_refresh: Callback<()>,
}

#[function_component]
pub fn SnapshotList(props: &Props) -> Html {
    html! {
        <div class="bg-gray-800 rounded-lg p-4">
            <div class="flex items-center justify-between mb-4">
                <h3 class="text-lg font-medium text-white">{"Snapshots"}</h3>
                <button
                    onclick={let cb = props.on_refresh.clone(); move |_| cb.emit(())}
                    disabled={props.loading}
                    class="px-3 py-1 bg-cyan-600 hover:bg-cyan-700 disabled:bg-gray-600 text-white text-sm rounded-md transition-colors"
                >
                    {if props.loading { "Loading..." } else { "Refresh" }}
                </button>
            </div>

            if props.snapshots.is_empty() && !props.loading {
                <p class="text-gray-400 text-sm">{"No snapshots found"}</p>
            } else {
                <div class="overflow-x-auto">
                    <table class="w-full text-sm text-left">
                        <thead class="bg-gray-700 text-gray-300">
                            <tr>
                                <th class="px-4 py-2">{"ID"}</th>
                                <th class="px-4 py-2">{"Time"}</th>
                                <th class="px-4 py-2">{"Tags"}</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-gray-700">
                            {props.snapshots.iter().map(|snap| {
                                html! {
                                    <tr key={snap.id.clone()} class="text-gray-200 hover:bg-gray-700">
                                        <td class="px-4 py-2 font-mono text-xs">{&snap.id}</td>
                                        <td class="px-4 py-2">{&snap.time}</td>
                                        <td class="px-4 py-2">
                                            {snap.tags.iter().map(|t| {
                                                html! { <span key={t.clone()} class="inline-block bg-gray-600 px-2 py-0.5 rounded text-xs mr-1">{t}</span> }
                                            }).collect::<Html>()}
                                        </td>
                                    </tr>
                                }
                            }).collect::<Html>()}
                        </tbody>
                    </table>
                </div>
            }
        </div>
    }
}