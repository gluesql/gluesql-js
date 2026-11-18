use {js_sys::Reflect, wasm_bindgen::JsValue};

pub fn set_panic_hook() {
    // When the `console_error_panic_hook` feature is enabled, we can call the
    // `set_panic_hook` function at least once during initialization, and then
    // we will get better error messages if our code ever panics.
    //
    // For more details see
    // https://github.com/rustwasm/console_error_panic_hook#readme
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

/// Reads `schema` from the options of `dump({ schema: false })`. It is `true`
/// unless the options object says otherwise.
pub fn dump_with_schema(options: &JsValue) -> bool {
    Reflect::get(options, &JsValue::from_str("schema"))
        .ok()
        .and_then(|schema| schema.as_bool())
        .unwrap_or(true)
}
