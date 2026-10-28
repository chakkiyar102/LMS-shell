use serde::{de::DeserializeOwned, Serialize};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"], catch)]
    async fn invoke(cmd: &str, args: JsValue) -> Result<JsValue, JsValue>;
}

/// Calls a Tauri command. Commands use snake_case argument names.
pub async fn call<R: DeserializeOwned>(cmd: &str, args: impl Serialize) -> Result<R, String> {
    let args = args
        .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
        .map_err(|e| e.to_string())?;
    let v = invoke(cmd, args).await.map_err(|e| e.as_string().unwrap_or_else(|| format!("{e:?}")))?;
    serde_wasm_bindgen::from_value(v).map_err(|e| e.to_string())
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "event"], js_name = listen)]
    async fn tauri_listen(event: &str, handler: &Closure<dyn Fn(JsValue)>) -> JsValue;
}

#[derive(serde::Deserialize)]
struct Event<T> { payload: T }

/// Subscribes to a backend event for the life of the app.
pub async fn listen<T: DeserializeOwned + 'static>(event: &str, f: impl Fn(T) + 'static) {
    let cb = Closure::<dyn Fn(JsValue)>::new(move |v: JsValue| {
        if let Ok(e) = serde_wasm_bindgen::from_value::<Event<T>>(v) { f(e.payload) }
    });
    tauri_listen(event, &cb).await;
    cb.forget(); // app-lifetime listener, never unsubscribed
}
