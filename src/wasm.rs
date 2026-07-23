use {
    crate::{payload::convert_to_js_value, utils},
    gluesql_composite_storage::CompositeStorage,
    gluesql_core::prelude::Glue as CoreGlue,
    gluesql_memory_storage::MemoryStorage,
    gluesql_web_storage::{WebStorage, WebStorageType},
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
    inner: CoreGlue<CompositeStorage>,
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

        let mut storage = CompositeStorage::default();
        storage.push("memory", MemoryStorage::default());
        storage.push("localStorage", WebStorage::new(WebStorageType::Local));
        storage.push("sessionStorage", WebStorage::new(WebStorageType::Session));
        storage.set_default("memory");

        debug("[GlueSQL] loaded: memory, localStorage, sessionStorage");
        debug("[GlueSQL] default engine: memory");
        debug("[GlueSQL] hello :)");

        Self {
            inner: CoreGlue::new(storage),
        }
    }

    #[wasm_bindgen(js_name = setDefaultEngine)]
    pub fn set_default_engine(&mut self, default_engine: String) -> Result<(), JsValue> {
        if ["memory", "localStorage", "sessionStorage"]
            .iter()
            .any(|engine| engine == &default_engine.as_str())
        {
            self.inner.storage.set_default(default_engine);

            Ok(())
        } else {
            Err(JsValue::from_str(
                format!("{default_engine} is not supported (options: memory, localStorage, sessionStorage)").as_str()
            ))
        }
    }

    pub fn query(&mut self, sql: String) -> Promise {
        match self.inner.execute(sql) {
            Ok(payloads) => Promise::resolve(&convert_to_js_value(payloads)),
            Err(error) => Promise::reject(&JsValue::from_str(&error.to_string())),
        }
    }
}
