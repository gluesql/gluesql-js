use {
    crate::{payload::convert_to_js_value, utils},
    gluesql_core::prelude::Glue as CoreGlue,
    gluesql_memory_storage::MemoryStorage,
    js_sys::Promise,
    wasm_bindgen::prelude::*,
};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn debug(s: &str);
}

#[wasm_bindgen]
pub struct Glue {
    inner: CoreGlue<MemoryStorage>,
}

impl Default for Glue {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(clippy::unused_unit)]
#[wasm_bindgen]
impl Glue {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        utils::set_panic_hook();

        debug("[GlueSQL] opfs build loaded (scaffolding storage: memory)");

        Self {
            inner: CoreGlue::new(MemoryStorage::default()),
        }
    }

    pub fn query(&mut self, sql: String) -> Promise {
        match self.inner.execute(sql) {
            Ok(payloads) => Promise::resolve(&convert_to_js_value(payloads)),
            Err(error) => Promise::reject(&JsValue::from_str(&error.to_string())),
        }
    }
}
