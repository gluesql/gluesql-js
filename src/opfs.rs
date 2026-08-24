use {
    crate::{payload::convert_to_js_value, utils},
    gluesql_core::prelude::Glue as CoreGlue,
    gluesql_opfs_storage::RedbStorage,
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
    inner: CoreGlue<RedbStorage>,
}

#[wasm_bindgen]
pub async fn load(namespace: Option<String>) -> Result<Glue, JsValue> {
    utils::set_panic_hook();

    let namespace = namespace.unwrap_or_else(|| "gluesql".to_owned());
    let storage = gluesql_opfs_storage::open(&namespace)
        .await
        .map_err(|error| JsValue::from_str(&error.to_string()))?;

    debug(&format!("[GlueSQL] opfs build loaded: {namespace}"));

    Ok(Glue {
        inner: CoreGlue::new(storage),
    })
}

#[allow(clippy::unused_unit)]
#[wasm_bindgen]
impl Glue {
    pub fn query(&mut self, sql: String) -> Promise {
        match self.inner.execute(sql) {
            Ok(payloads) => Promise::resolve(&convert_to_js_value(payloads)),
            Err(error) => Promise::reject(&JsValue::from_str(&error.to_string())),
        }
    }
}
