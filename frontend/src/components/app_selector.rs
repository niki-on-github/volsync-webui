use crate::api::App;
use yew::prelude::*;

#[derive(Properties, Clone, PartialEq)]
pub struct Props {
    pub selected: Option<App>,
    pub apps: Vec<App>,
    pub on_select: Callback<Option<App>>,
}

#[function_component]
pub fn AppSelector(props: &Props) -> Html {
    let selected_val = props.selected.as_ref().map(|a| format!("{}/{}", a.name, a.namespace)).unwrap_or_default();

    let on_change = {
        let apps = props.apps.clone();
        let on_select = props.on_select.clone();
        Callback::from(move |e: Event| {
            let val = e.target_unchecked_into::<web_sys::HtmlSelectElement>().value();
            let app = apps.iter().find(|a| format!("{}/{}", a.name, a.namespace) == val).cloned();
            on_select.emit(app);
        })
    };

    html! {
        <div class="flex items-center gap-2">
            <label class="text-sm font-medium text-gray-300">{"Application:"}</label>
            <select
                class="bg-gray-700 border border-gray-600 rounded-md px-3 py-2 text-white text-sm focus:outline-none focus:ring-2 focus:ring-cyan-500 min-w-[200px]"
                value={selected_val}
                onchange={on_change}
            >
                <option value="">{"Select an app..."}</option>
                {props.apps.iter().map(|app| {
                    let val = format!("{}/{}", app.name, app.namespace);
                    let label = format!("{} ({})", app.name, app.namespace);
                    html! { <option value={val}>{label}</option> }
                }).collect::<Html>()}
            </select>
        </div>
    }
}