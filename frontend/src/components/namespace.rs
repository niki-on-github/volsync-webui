use yew::prelude::*;

#[derive(Properties, Clone, PartialEq)]
pub struct Props {
    pub selected: String,
    pub namespaces: Vec<String>,
    pub on_select: Callback<String>,
}

#[function_component]
pub fn NamespaceSelector(props: &Props) -> Html {
    html! {
        <div class="flex items-center gap-2">
            <label class="text-sm font-medium text-gray-300">{"Namespace:"}</label>
            <select
                class="bg-gray-700 border border-gray-600 rounded-md px-3 py-2 text-white text-sm focus:outline-none focus:ring-2 focus:ring-cyan-500"
                value={props.selected.clone()}
                onchange={let on_select = props.on_select.clone(); move |e: Event| {
                    let val = e.target_unchecked_into::<web_sys::HtmlSelectElement>().value();
                    on_select.emit(val);
                }}
            >
                <option value="">{"All Namespaces"}</option>
                {props.namespaces.iter().map(|ns| {
                    html! { <option key={ns.clone()} value={ns.clone()}>{ns.clone()}</option> }
                }).collect::<Html>()}
            </select>
        </div>
    }
}