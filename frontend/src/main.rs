fn main() {
    wasm_logger::init(wasm_logger::Config::default());
    yew::Renderer::<volsync_webui_frontend::AppComponent>::new().render();
}
