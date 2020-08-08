mod convert;
mod web_storage;
mod memory_storage;
mod utils;

use wasm_bindgen::prelude::*;

use convert::convert;

pub use memory_storage::MemoryStorage;
pub use web_storage::{LocalStorage, SessionStorage};

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

#[wasm_bindgen]
#[derive(Default)]
pub struct Glue {
    storage: Option<MemoryStorage>,
}

#[wasm_bindgen]
impl Glue {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        utils::set_panic_hook();

        log("[GlueSQL] :)");

        let storage = Some(MemoryStorage::new().unwrap());

        Self { storage }
    }

    pub fn execute(&mut self, sql: String) -> Result<JsValue, JsValue> {
        let mut payloads = vec![];

        let queries = gluesql_core::parse(&sql).map_err(|error| {
            let message = format!("{:?}", error);

            JsValue::from_serde(&message).unwrap()
        })?;

        for query in queries.iter() {
            match gluesql_core::execute(self.storage.take().unwrap(), query) {
                Ok((storage, payload)) => {
                    self.storage = Some(storage);

                    payloads.push(payload);
                }
                Err((storage, error)) => {
                    self.storage = Some(storage);

                    return Err(JsValue::from_serde(&error).unwrap());
                }
            }
        }

        Ok(convert(payloads))
    }
}
